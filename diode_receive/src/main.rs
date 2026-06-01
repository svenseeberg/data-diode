use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(feature = "arduino")]
use std::time::{Duration, Instant};

use clap::Parser;
use log::{error, info, warn};

use diode_common::hash::md5_hex;
use diode_common::logging::init_logger;
use diode_common::proto::{
    BATCH_DATA_CHUNKS, BATCH_PARITY_CHUNKS, CHUNK_SIZE, HEADER_SIZE, PKG_TYPE_DATA, PKG_TYPE_END,
    PKG_TYPE_PARITY, PKG_TYPE_START, PKG_TYPE_UDP, SHARD_SIZE, decode_parity_field,
};

/// Safety cap on memory held for the current batch. Normal operation uses
/// far less (~1 MB per batch); this only fires if something is wrong.
const BUFFER_BYTE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Parser, Debug, Clone)]
#[command(about = "Receive files through unidirectional serial device")]
struct Args {
    #[arg(long, default_value = "10.125.125.255", help = "Bind to broadcast address / subnet")]
    bind_subnet: String,

    #[arg(long, default_value_t = 5005, help = "Bind port")]
    bind_port: u16,

    #[arg(long, help = "directory to which received files are written")]
    directory: String,

    #[arg(long, help = "serial device, i.e. /dev/cuaU1")]
    arduino: Option<String>,

    #[arg(long, help = "Target port for UDP packet forwarding.")]
    udp_target_port: Option<u16>,

    #[arg(long, default_value = "127.0.0.1", help = "Target ip for UDP packet forwarding.")]
    udp_target_ip: String,

    #[arg(
        long,
        default_value_t = 8 * 1024 * 1024,
        help = "Requested SO_RCVBUF size in bytes (best-effort, capped by kernel)"
    )]
    rcvbuf: i32,
}

#[derive(Clone)]
struct Packet {
    pkg_type: u8,
    count: u64,
    path_hash: Vec<u8>,
    data_hash: Vec<u8>,
    data: Vec<u8>,
}

fn decode_packet(raw: &[u8]) -> Option<Packet> {
    if raw.len() < HEADER_SIZE {
        return None;
    }
    let pkg_type = std::str::from_utf8(&raw[0..1]).ok()?.parse::<u8>().ok()?;
    let count = std::str::from_utf8(&raw[1..11]).ok()?.parse::<u64>().ok()?;
    Some(Packet {
        pkg_type,
        count,
        path_hash: raw[11..43].to_vec(),
        data_hash: raw[43..75].to_vec(),
        data: raw[75..].to_vec(),
    })
}

struct ReceiveState {
    bytes_transferred: u64,
    file_path: Option<String>,
    file_size: u64,
    parity_per_batch: u64,
    path_hash: Option<Vec<u8>>,
    /// Index of the batch currently being collected.
    current_batch: u64,
    /// Data chunks for the current batch, keyed by absolute chunk count.
    data_buffer: BTreeMap<u64, Vec<u8>>,
    /// Parity shards for the current batch, keyed by index within the batch.
    parity_buffer: BTreeMap<u64, Vec<u8>>,
    bytes_buffered: usize,
    failed: bool,
    output: Option<File>,
}

impl ReceiveState {
    fn new() -> Self {
        Self {
            bytes_transferred: 0,
            file_path: None,
            file_size: 0,
            parity_per_batch: 0,
            path_hash: None,
            current_batch: 0,
            data_buffer: BTreeMap::new(),
            parity_buffer: BTreeMap::new(),
            bytes_buffered: 0,
            failed: false,
            output: None,
        }
    }
}

struct Stats {
    files_transferred: usize,
    files_failed: usize,
    total_transferred: u64,
}

impl Stats {
    fn new() -> Self {
        Self {
            files_transferred: 0,
            files_failed: 0,
            total_transferred: 0,
        }
    }
}

fn is_chunk_valid(packet: &Packet, state: &ReceiveState) -> bool {
    if let Some(ph) = &state.path_hash {
        if &packet.path_hash != ph {
            warn!("Path hash mismatch in chunk {}", packet.count);
            return false;
        }
    }
    let expected = md5_hex(&packet.data);
    if expected.as_bytes() != packet.data_hash.as_slice() {
        warn!("Data hash mismatch in chunk {}", packet.count);
        return false;
    }
    true
}

fn batch_for_data(count: u64) -> u64 {
    count / BATCH_DATA_CHUNKS as u64
}

fn batch_for_parity(count: u64) -> u64 {
    count / BATCH_PARITY_CHUNKS as u64
}

fn batch_data_size(batch_idx: u64, file_size: u64) -> usize {
    let total_chunks = file_size.div_ceil(CHUNK_SIZE as u64) as usize;
    let start = batch_idx as usize * BATCH_DATA_CHUNKS;
    if start >= total_chunks {
        0
    } else {
        (total_chunks - start).min(BATCH_DATA_CHUNKS)
    }
}

fn buffer_data(state: &mut ReceiveState, stats: &mut Stats, packet: Packet) {
    if state.data_buffer.contains_key(&packet.count) {
        return;
    }
    if state.bytes_buffered.saturating_add(packet.data.len()) > BUFFER_BYTE_LIMIT {
        error!(
            "Buffer size limit ({} bytes) exceeded for {}",
            BUFFER_BYTE_LIMIT,
            state.file_path.as_deref().unwrap_or("?")
        );
        state.failed = true;
        return;
    }
    state.bytes_buffered += packet.data.len();
    state.bytes_transferred += packet.data.len() as u64;
    stats.total_transferred += packet.data.len() as u64;
    state.data_buffer.insert(packet.count, packet.data);
}

fn buffer_parity(state: &mut ReceiveState, packet: Packet, idx_in_batch: u64) {
    if state.parity_buffer.contains_key(&idx_in_batch) {
        return;
    }
    if state.bytes_buffered.saturating_add(packet.data.len()) > BUFFER_BYTE_LIMIT {
        error!(
            "Buffer size limit ({} bytes) exceeded for {}",
            BUFFER_BYTE_LIMIT,
            state.file_path.as_deref().unwrap_or("?")
        );
        state.failed = true;
        return;
    }
    state.bytes_buffered += packet.data.len();
    state.parity_buffer.insert(idx_in_batch, packet.data);
}

fn reset_transfer(state: &mut ReceiveState, stats: &mut Stats, success: bool) {
    if let Some(p) = state.file_path.take() {
        if success {
            stats.files_transferred += 1;
            info!("Finished receiving {}", p);
        } else {
            stats.files_failed += 1;
        }
    }
    *state = ReceiveState::new();
}

fn mark_failure(output_dir: &Path, rel_path: &str) {
    let _ = fs::remove_file(output_dir.join(rel_path));
    let failed_path = output_dir.join(format!("{}.failed", rel_path));
    if let Some(parent) = failed_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&failed_path);
}

fn partial_path(output_dir: &Path, rel_path: &str) -> PathBuf {
    output_dir.join(format!("{}.partial", rel_path))
}

fn open_output_file(output_dir: &Path, rel_path: &str, file_size: u64) -> std::io::Result<File> {
    let full = partial_path(output_dir, rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    let f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&full)?;
    if file_size > 0 {
        f.set_len(file_size)?;
    }
    Ok(f)
}

fn new_file(
    state: &mut ReceiveState,
    stats: &mut Stats,
    output_dir: &Path,
    path: String,
    size: u64,
    path_hash: Vec<u8>,
    parity_per_batch: u64,
) {
    if state.file_path.as_deref() == Some(path.as_str()) {
        return;
    }
    if state.file_path.is_some() {
        error!(
            "Unfinished previous transfer of {} failed",
            state.file_path.as_deref().unwrap_or("?")
        );
        let prev = state.file_path.clone().unwrap();
        state.output.take();
        let _ = fs::remove_file(partial_path(output_dir, &prev));
        mark_failure(output_dir, &prev);
        reset_transfer(state, stats, false);
    }
    let _ = fs::remove_file(output_dir.join(&path));
    let _ = fs::remove_file(partial_path(output_dir, &path));
    let _ = fs::remove_file(output_dir.join(format!("{}.failed", path)));
    let output = match open_output_file(output_dir, &path, size) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open output file {}: {}", path, e);
            stats.files_failed += 1;
            mark_failure(output_dir, &path);
            return;
        }
    };
    state.file_path = Some(path.clone());
    state.path_hash = Some(path_hash);
    state.file_size = size;
    state.parity_per_batch = parity_per_batch;
    state.current_batch = 0;
    state.output = Some(output);
    info!(
        "Receiving file {} (size {}, parity per batch {})",
        path, size, parity_per_batch
    );
}

/// Reconstruct the current batch from buffered data + parity and write it to
/// `state.output`. Clears the per-batch buffers afterwards. Returns Err if the
/// batch cannot be reconstructed or written.
fn finalize_current_batch(state: &mut ReceiveState) -> Result<(), String> {
    let batch_idx = state.current_batch;
    let file_size = state.file_size;
    let parity_per_batch = state.parity_per_batch as usize;
    let data_size = batch_data_size(batch_idx, file_size);
    if data_size == 0 {
        state.data_buffer.clear();
        state.parity_buffer.clear();
        state.bytes_buffered = 0;
        return Ok(());
    }

    let total_chunks = file_size.div_ceil(CHUNK_SIZE as u64) as usize;
    let remainder = (file_size % CHUNK_SIZE as u64) as usize;
    let last_chunk_size_overall = if remainder == 0 { CHUNK_SIZE } else { remainder };
    let batch_start_chunk = batch_idx as usize * BATCH_DATA_CHUNKS;

    let data_received = state.data_buffer.len();
    let parity_received = state.parity_buffer.len();

    let restored: BTreeMap<usize, Vec<u8>> = if data_received >= data_size {
        BTreeMap::new()
    } else {
        if parity_per_batch == 0 {
            return Err(format!(
                "Batch {} missing {} chunks and FEC is disabled",
                batch_idx,
                data_size - data_received
            ));
        }
        if data_received + parity_received < data_size {
            return Err(format!(
                "Batch {} insufficient: {} data + {} parity, need {}",
                batch_idx, data_received, parity_received, data_size
            ));
        }
        let mut decoder =
            reed_solomon_simd::ReedSolomonDecoder::new(data_size, parity_per_batch, SHARD_SIZE)
                .map_err(|e| format!("FEC decoder for batch {}: {}", batch_idx, e))?;
        let mut padded = vec![0u8; SHARD_SIZE];
        for (&abs_idx, data) in &state.data_buffer {
            let idx_in_batch = (abs_idx as usize).saturating_sub(batch_start_chunk);
            if idx_in_batch >= data_size {
                continue;
            }
            padded.iter_mut().for_each(|b| *b = 0);
            let copy_len = data.len().min(SHARD_SIZE);
            padded[..copy_len].copy_from_slice(&data[..copy_len]);
            decoder
                .add_original_shard(idx_in_batch, &padded)
                .map_err(|e| format!("FEC add_original[{}]: {}", idx_in_batch, e))?;
        }
        for (&idx_in_batch, data) in &state.parity_buffer {
            let idx = idx_in_batch as usize;
            if idx >= parity_per_batch || data.len() < SHARD_SIZE {
                continue;
            }
            decoder
                .add_recovery_shard(idx, &data[..SHARD_SIZE])
                .map_err(|e| format!("FEC add_recovery[{}]: {}", idx, e))?;
        }
        let result = decoder
            .decode()
            .map_err(|e| format!("FEC decode batch {}: {}", batch_idx, e))?;
        let map: BTreeMap<usize, Vec<u8>> = result
            .restored_original_iter()
            .map(|(idx, shard)| (idx, shard.to_vec()))
            .collect();
        info!(
            "FEC reconstructed {} chunk(s) for batch {}",
            map.len(),
            batch_idx
        );
        map
    };

    let file = state
        .output
        .as_mut()
        .ok_or_else(|| "No output file open".to_string())?;
    let offset = batch_start_chunk as u64 * CHUNK_SIZE as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("seek to {}: {}", offset, e))?;

    for i in 0..data_size {
        let abs_idx = (batch_start_chunk + i) as u64;
        let is_final_chunk = batch_start_chunk + i + 1 == total_chunks;
        let expected_len = if is_final_chunk {
            last_chunk_size_overall
        } else {
            CHUNK_SIZE
        };
        let chunk: &[u8] = if let Some(data) = state.data_buffer.get(&abs_idx) {
            if data.len() < expected_len {
                return Err(format!(
                    "Chunk {} too short: {} < {}",
                    abs_idx,
                    data.len(),
                    expected_len
                ));
            }
            &data[..expected_len]
        } else if let Some(shard) = restored.get(&i) {
            if shard.len() < expected_len {
                return Err(format!(
                    "Restored chunk {} too short: {} < {}",
                    abs_idx,
                    shard.len(),
                    expected_len
                ));
            }
            &shard[..expected_len]
        } else {
            return Err(format!("Chunk {} still missing after reconstruction", abs_idx));
        };
        file.write_all(chunk).map_err(|e| format!("write: {}", e))?;
    }

    state.data_buffer.clear();
    state.parity_buffer.clear();
    state.bytes_buffered = 0;
    Ok(())
}

/// Finalize the current batch (and any empty intermediate batches) until
/// `current_batch == target_batch`. If a finalization fails, mark `failed`
/// and stop advancing.
fn advance_to_batch(state: &mut ReceiveState, target_batch: u64) {
    while !state.failed && state.current_batch < target_batch {
        if let Err(e) = finalize_current_batch(state) {
            error!(
                "Batch {} finalize failed for {}: {}",
                state.current_batch,
                state.file_path.as_deref().unwrap_or("?"),
                e
            );
            state.failed = true;
            return;
        }
        state.current_batch += 1;
    }
}

fn finish_file(state: &mut ReceiveState, stats: &mut Stats, output_dir: &Path) {
    let rel_path = match state.file_path.clone() {
        Some(p) => p,
        None => return,
    };

    if !state.failed {
        if let Err(e) = finalize_current_batch(state) {
            error!("Final batch failed for {}: {}", rel_path, e);
            state.failed = true;
        }
    }

    if let Some(mut f) = state.output.take() {
        let _ = f.flush();
    }

    let partial = partial_path(output_dir, &rel_path);

    if state.failed {
        let _ = fs::remove_file(&partial);
        mark_failure(output_dir, &rel_path);
        reset_transfer(state, stats, false);
        return;
    }

    let final_path = output_dir.join(&rel_path);
    if let Err(e) = fs::rename(&partial, &final_path) {
        error!(
            "Failed to rename {} -> {}: {}",
            partial.display(),
            final_path.display(),
            e
        );
        let _ = fs::remove_file(&partial);
        mark_failure(output_dir, &rel_path);
        reset_transfer(state, stats, false);
        return;
    }

    let failed_path = output_dir.join(format!("{}.failed", rel_path));
    if failed_path.exists() {
        let _ = fs::remove_file(&failed_path);
    }
    reset_transfer(state, stats, true);
}

struct UdpForwardState {
    current_id: Option<Vec<u8>>,
    full_packet: Vec<u8>,
    previous_count: u64,
}

fn forward_udp_packets(args: Args, rx: mpsc::Receiver<Packet>) {
    let udp_sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to bind UDP forward socket: {}", e);
            return;
        }
    };
    let port = match args.udp_target_port {
        Some(p) => p,
        None => return,
    };
    info!("Forwarding UDP packets to {}:{}", args.udp_target_ip, port);
    let target = format!("{}:{}", args.udp_target_ip, port);
    let mut fwd = UdpForwardState {
        current_id: None,
        full_packet: Vec::new(),
        previous_count: 0,
    };
    while let Ok(packet) = rx.recv() {
        let size: usize = match std::str::from_utf8(&packet.path_hash)
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            Some(s) => s,
            None => continue,
        };
        let id = packet.data_hash[..8].to_vec();
        let count = packet.count;
        if count == 0 && fwd.current_id.is_none() {
            fwd.full_packet = packet.data.clone();
            fwd.previous_count = 0;
            fwd.current_id = Some(id.clone());
        } else if Some(&id) == fwd.current_id.as_ref() && count == fwd.previous_count + 1 {
            fwd.full_packet.extend_from_slice(&packet.data);
            fwd.previous_count = count;
        }
        if fwd.full_packet.len() == size {
            if let Err(e) = udp_sock.send_to(&fwd.full_packet, target.as_str()) {
                error!("UDP forward send error: {}", e);
            }
            fwd.previous_count = 0;
            fwd.full_packet.clear();
            fwd.current_id = None;
        }
    }
}

#[cfg(feature = "arduino")]
struct Display {
    port: Box<dyn serialport::SerialPort>,
    last_update: Instant,
}

#[cfg(feature = "arduino")]
impl Display {
    /// Minimum interval between display updates. The HD44780 over I2C runs at
    /// roughly 500 chars/s while the serial link delivers ~3840 bytes/s, so
    /// back-to-back updates can overflow the Arduino's 64-byte serial buffer.
    /// A lost `\n` desyncs the sketch's row toggle and the display ends up
    /// printing line 1 onto row 1 (and vice versa) for the rest of the run.
    const MIN_INTERVAL: Duration = Duration::from_millis(250);

    fn new(device: &str) -> Option<Self> {
        match serialport::new(device, 38400).timeout(Duration::from_secs(1)).open() {
            Ok(mut port) => {
                // Opening the serial port toggles DTR, which resets most
                // Arduino boards. Wait for the bootloader to hand control
                // back to the sketch before sending anything.
                thread::sleep(Duration::from_secs(2));
                let _ = port.write_all(b"\r                ");
                let _ = port.write_all(b"\n                ");
                let _ = port.flush();
                Some(Self {
                    port,
                    last_update: Instant::now() - Self::MIN_INTERVAL,
                })
            }
            Err(e) => {
                error!("Failed to open serial port {}: {}", device, e);
                None
            }
        }
    }

    fn update(&mut self, stats: &Stats, state: &ReceiveState, force: bool) {
        let now = Instant::now();
        if !force && now.duration_since(self.last_update) < Self::MIN_INTERVAL {
            return;
        }
        self.last_update = now;

        let data_mb = (stats.total_transferred as f64) / 1024.0 / 1024.0;
        let (state_str, progress_str) = if state.file_path.is_some() {
            let progress = if state.file_size == 0 {
                100
            } else {
                ((state.bytes_transferred as f64 / state.file_size as f64) * 100.0) as i32
            };
            ("Rx   ", format!(" {:>3}%", progress))
        } else {
            ("Wait ", "     ".to_string())
        };
        let line1_raw = format!(
            "{}{:>3}F {:>2}E",
            state_str, stats.files_transferred, stats.files_failed
        );
        let line2_raw = format!("{:>5.1}MB{}", data_mb, progress_str);
        let line1 = format!("{:<16.16}", line1_raw);
        let line2 = format!("{:<16.16}", line2_raw);
        // '\r' resets the sketch's row tracker so line1 always lands on row 0
        // even if a previous update lost a byte. '\n' between the lines moves
        // the cursor to row 1 for line2.
        let _ = self.port.write_all(b"\r");
        let _ = self.port.write_all(line1.as_bytes());
        let _ = self.port.write_all(b"\n");
        let _ = self.port.write_all(line2.as_bytes());
        let _ = self.port.flush();
    }
}

fn main() {
    let args = Args::parse();
    init_logger();

    let output_dir = PathBuf::from(&args.directory);
    if let Err(e) = fs::create_dir_all(&output_dir) {
        error!("Failed to create directory {}: {}", output_dir.display(), e);
        std::process::exit(1);
    }

    let sock = match UdpSocket::bind((args.bind_subnet.as_str(), args.bind_port)) {
        Ok(s) => s,
        Err(e) => {
            error!(
                "Failed to bind socket {}:{} - {}",
                args.bind_subnet, args.bind_port, e
            );
            std::process::exit(1);
        }
    };
    let _ = sock.set_broadcast(true);
    tune_rcvbuf(&sock, args.rcvbuf);

    let (pkt_tx, pkt_rx) = mpsc::channel::<Vec<u8>>();
    let (udp_tx, udp_rx) = mpsc::channel::<Packet>();

    let listen_sock = sock.try_clone().expect("clone socket");
    thread::spawn(move || {
        let mut buf = vec![0u8; 65535];
        loop {
            match listen_sock.recv_from(&mut buf) {
                Ok((n, _)) => {
                    if pkt_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => error!("Recv error: {}", e),
            }
        }
    });

    let udp_forwarding_enabled = args.udp_target_port.is_some();
    if udp_forwarding_enabled {
        let args_clone = args.clone();
        thread::spawn(move || forward_udp_packets(args_clone, udp_rx));
    }

    let stats = Arc::new(Mutex::new(Stats::new()));
    let state = Arc::new(Mutex::new(ReceiveState::new()));

    #[cfg(feature = "arduino")]
    let display = args.arduino.as_deref().and_then(Display::new);
    #[cfg(feature = "arduino")]
    let display = Arc::new(Mutex::new(display));
    #[cfg(not(feature = "arduino"))]
    {
        if args.arduino.is_some() {
            warn!("Arduino support requested but binary was built without the 'arduino' feature");
        }
    }

    #[cfg(feature = "arduino")]
    {
        if let Ok(mut d) = display.lock() {
            if let Some(d) = d.as_mut() {
                d.update(&stats.lock().unwrap(), &state.lock().unwrap(), true);
            }
        }
    }

    let output_dir_worker = output_dir.clone();
    let stats_worker = Arc::clone(&stats);
    let state_worker = Arc::clone(&state);
    #[cfg(feature = "arduino")]
    let display_worker = Arc::clone(&display);

    let worker = thread::spawn(move || {
        while let Ok(raw) = pkt_rx.recv() {
            let packet = match decode_packet(&raw) {
                Some(p) => p,
                None => {
                    error!("Failed to decode packet");
                    continue;
                }
            };
            let pkt_type = packet.pkg_type;
            let count = packet.count;
            let mut state_guard = state_worker.lock().unwrap();
            let mut stats_guard = stats_worker.lock().unwrap();
            match pkt_type {
                PKG_TYPE_START => {
                    let path = String::from_utf8_lossy(&packet.data).into_owned();
                    let parity_per_batch =
                        decode_parity_field(&packet.data_hash).unwrap_or(0);
                    new_file(
                        &mut state_guard,
                        &mut stats_guard,
                        &output_dir_worker,
                        path,
                        packet.count,
                        packet.path_hash.clone(),
                        parity_per_batch,
                    );
                }
                PKG_TYPE_DATA if state_guard.file_path.is_some() => {
                    if !state_guard.failed && is_chunk_valid(&packet, &state_guard) {
                        let batch = batch_for_data(packet.count);
                        if batch > state_guard.current_batch {
                            advance_to_batch(&mut state_guard, batch);
                        }
                        if !state_guard.failed && batch == state_guard.current_batch {
                            buffer_data(&mut state_guard, &mut stats_guard, packet);
                        }
                    }
                }
                PKG_TYPE_PARITY if state_guard.file_path.is_some() => {
                    if !state_guard.failed && is_chunk_valid(&packet, &state_guard) {
                        let batch = batch_for_parity(packet.count);
                        if batch > state_guard.current_batch {
                            advance_to_batch(&mut state_guard, batch);
                        }
                        if !state_guard.failed && batch == state_guard.current_batch {
                            let idx_in_batch =
                                packet.count % BATCH_PARITY_CHUNKS as u64;
                            buffer_parity(&mut state_guard, packet, idx_in_batch);
                        }
                    }
                }
                PKG_TYPE_END if state_guard.file_path.is_some() => {
                    finish_file(&mut state_guard, &mut stats_guard, &output_dir_worker);
                }
                PKG_TYPE_UDP if udp_forwarding_enabled => {
                    stats_guard.total_transferred += packet.data.len() as u64;
                    let _ = udp_tx.send(packet);
                }
                _ => {}
            }
            drop(state_guard);
            drop(stats_guard);
            #[cfg(feature = "arduino")]
            {
                let force = matches!(pkt_type, PKG_TYPE_START | PKG_TYPE_END);
                if force || count % 1000 == 0 {
                    if let Ok(mut d) = display_worker.lock() {
                        if let Some(d) = d.as_mut() {
                            d.update(
                                &stats_worker.lock().unwrap(),
                                &state_worker.lock().unwrap(),
                                force,
                            );
                        }
                    }
                }
            }
            #[cfg(not(feature = "arduino"))]
            {
                let _ = count;
            }
        }
    });

    let _ = worker.join();
}

fn tune_rcvbuf(sock: &UdpSocket, want: i32) {
    let fd = sock.as_raw_fd();
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &want as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        warn!(
            "setsockopt(SO_RCVBUF, {}) failed: {}",
            want,
            std::io::Error::last_os_error()
        );
    }

    let mut got: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &mut got as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc == 0 {
        info!("SO_RCVBUF requested {} bytes, kernel granted {} bytes", want, got);
    } else {
        warn!(
            "getsockopt(SO_RCVBUF) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}
