use std::fs;
use std::io::{BufReader, Read};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use log::{error, info, warn};

use diode_common::hash::md5_hex;
use diode_common::logging::init_logger;
use diode_common::proto::{
    BATCH_DATA_CHUNKS, BATCH_PARITY_CHUNKS, CHUNK_SIZE, EMPTY_HASH, HEADER_SIZE, PKG_TYPE_DATA,
    PKG_TYPE_END, PKG_TYPE_PARITY, PKG_TYPE_START, PKG_TYPE_UDP, SHARD_SIZE, encode_packet,
    encode_packet_into, encode_parity_field,
};

const SCAN_INTERVAL: Duration = Duration::from_secs(2);
const STABILITY_CHECK_DELAY: Duration = Duration::from_secs(1);
const CONTROL_RESEND: usize = 3;
const UDP_RESEND: usize = 2;
const ENOBUFS_INITIAL_BACKOFF: Duration = Duration::from_millis(1);
const FILE_READ_BUFFER: usize = 64 * 1024;
const PACER_PACKET_BYTES: u64 = (HEADER_SIZE + CHUNK_SIZE) as u64;
const SLEEP_HEADROOM_NANOS: u64 = 200_000;

struct Pacer {
    base: Instant,
    interval_nanos: u64,
    next_deadline_nanos: AtomicU64,
}

impl Pacer {
    fn from_bitrate_mbps(mbps: f64) -> Self {
        let interval_nanos = if mbps <= 0.0 {
            0
        } else {
            let bits_per_packet = (PACER_PACKET_BYTES as f64) * 8.0;
            let secs_per_packet = bits_per_packet / (mbps * 1_000_000.0);
            (secs_per_packet * 1_000_000_000.0).round() as u64
        };
        Self {
            base: Instant::now(),
            interval_nanos,
            next_deadline_nanos: AtomicU64::new(0),
        }
    }

    fn wait(&self) {
        if self.interval_nanos == 0 {
            return;
        }
        let now_ns = self.base.elapsed().as_nanos() as u64;
        let mut prev = self.next_deadline_nanos.load(Ordering::Relaxed);
        let slot = loop {
            let target = prev.max(now_ns);
            let new = target.saturating_add(self.interval_nanos);
            match self.next_deadline_nanos.compare_exchange_weak(
                prev,
                new,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break target,
                Err(actual) => prev = actual,
            }
        };
        let mut now_ns = self.base.elapsed().as_nanos() as u64;
        while now_ns < slot {
            let remaining = slot - now_ns;
            if remaining > 1_000_000 {
                thread::sleep(Duration::from_nanos(remaining - SLEEP_HEADROOM_NANOS));
            } else {
                std::hint::spin_loop();
            }
            now_ns = self.base.elapsed().as_nanos() as u64;
        }
    }
}

#[derive(Clone)]
struct SendCfg {
    pacer: Arc<Pacer>,
    max_backoff: Duration,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Send files through unidirectional serial device")]
struct Args {
    #[arg(long, default_value = "10.125.125.255", help = "Target subnet broadcast IP")]
    target_subnet: String,

    #[arg(long, default_value_t = 5005, help = "Target port")]
    target_port: u16,

    #[arg(long, help = "directory from which to send files")]
    directory: String,

    #[arg(long, help = "UDP listen port. Incoming packets will be forwarded through the diode.")]
    udp_port: Option<u16>,

    #[arg(long, default_value = "127.0.0.1", help = "IP address to listen on for UDP packets.")]
    udp_host: String,

    #[arg(
        long,
        default_value_t = 90.0,
        help = "Target wire bitrate cap in Mbps (0 = unlimited). ~90 Mbps yields ~10 MB/s of payload."
    )]
    target_bitrate_mbps: f64,

    #[arg(
        long,
        default_value_t = 250,
        help = "Maximum backoff between ENOBUFS retries (milliseconds)"
    )]
    enobufs_max_backoff_ms: u64,

    #[arg(
        long,
        default_value_t = 2 * 1024 * 1024,
        help = "Requested SO_SNDBUF size in bytes (best-effort, capped by kernel)"
    )]
    sndbuf: i32,
}

fn send_to_blocking(
    sock: &UdpSocket,
    target: &SocketAddr,
    packet: &[u8],
    cfg: &SendCfg,
) -> std::io::Result<()> {
    let mut backoff = ENOBUFS_INITIAL_BACKOFF;
    loop {
        match sock.send_to(packet, target) {
            Ok(_) => {
                cfg.pacer.wait();
                return Ok(());
            }
            Err(e) => {
                let is_enobufs = matches!(e.raw_os_error(), Some(libc::ENOBUFS))
                    || e.kind() == std::io::ErrorKind::WouldBlock;
                if !is_enobufs {
                    return Err(e);
                }
                warn!("ENOBUFS, backing off {} ms", backoff.as_millis());
                thread::sleep(backoff);
                backoff = (backoff * 2).min(cfg.max_backoff);
            }
        }
    }
}

fn send_with_retry(
    sock: &UdpSocket,
    target: &SocketAddr,
    packet: &[u8],
    times: usize,
    cfg: &SendCfg,
) -> std::io::Result<()> {
    let n = times.max(1);
    for _ in 0..n {
        send_to_blocking(sock, target, packet, cfg)?;
    }
    Ok(())
}

fn send_start_packet(
    sock: &UdpSocket,
    target: &SocketAddr,
    filesize: u64,
    rel_path: &str,
    rel_path_hash: &str,
    parity_count: u64,
    cfg: &SendCfg,
) -> std::io::Result<()> {
    info!("Sending file: {}", rel_path);
    let packet = encode_packet(
        PKG_TYPE_START,
        filesize,
        rel_path_hash,
        &encode_parity_field(parity_count),
        rel_path.as_bytes(),
    );
    send_with_retry(sock, target, &packet, CONTROL_RESEND, cfg)
}

fn send_end_packet(
    sock: &UdpSocket,
    target: &SocketAddr,
    full_path: &Path,
    rel_path: &str,
    count: u64,
    cfg: &SendCfg,
) -> std::io::Result<()> {
    let packet = encode_packet(PKG_TYPE_END, count, EMPTY_HASH, EMPTY_HASH, rel_path.as_bytes());
    send_with_retry(sock, target, &packet, CONTROL_RESEND, cfg)?;
    fs::remove_file(full_path)?;
    info!("Finished sending file: {}", rel_path);
    Ok(())
}

fn send_data_packet(
    sock: &UdpSocket,
    target: &SocketAddr,
    rel_path_hash: &str,
    chunk: &[u8],
    count: u64,
    packet_buf: &mut Vec<u8>,
    cfg: &SendCfg,
) -> std::io::Result<()> {
    let data_hash = md5_hex(chunk);
    encode_packet_into(packet_buf, PKG_TYPE_DATA, count, rel_path_hash, &data_hash, chunk);
    send_to_blocking(sock, target, packet_buf, cfg)
}

fn send_parity_packets(
    sock: &UdpSocket,
    target: &SocketAddr,
    rel_path_hash: &str,
    data_payloads: &[Vec<u8>],
    parity_count: usize,
    batch_idx: u64,
    packet_buf: &mut Vec<u8>,
    cfg: &SendCfg,
) -> Result<(), Box<dyn std::error::Error>> {
    if data_payloads.is_empty() || parity_count == 0 {
        return Ok(());
    }

    let mut encoder = reed_solomon_simd::ReedSolomonEncoder::new(
        data_payloads.len(),
        parity_count,
        SHARD_SIZE,
    )?;

    let mut shard_buf = vec![0u8; SHARD_SIZE];
    for data in data_payloads {
        shard_buf.iter_mut().for_each(|b| *b = 0);
        shard_buf[..data.len()].copy_from_slice(data);
        encoder.add_original_shard(&shard_buf)?;
    }

    let result = encoder.encode()?;

    let base = batch_idx * parity_count as u64;
    for (idx, parity) in result.recovery_iter().enumerate() {
        let parity_hash = md5_hex(parity);
        encode_packet_into(
            packet_buf,
            PKG_TYPE_PARITY,
            base + idx as u64,
            rel_path_hash,
            &parity_hash,
            parity,
        );
        if let Err(e) = send_to_blocking(sock, target, packet_buf, cfg) {
            error!(
                "Failed to send parity packet (batch {}, idx {}): {}",
                batch_idx, idx, e
            );
        }
    }
    Ok(())
}

fn read_chunk(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}

fn send_file_chunks(
    sock: &UdpSocket,
    target: &SocketAddr,
    full_path: &Path,
    rel_path: &str,
    cfg: &SendCfg,
) -> std::io::Result<()> {
    let rel_path_hash = md5_hex(rel_path.as_bytes());
    let filesize = fs::metadata(full_path)?.len();

    send_start_packet(
        sock,
        target,
        filesize,
        rel_path,
        &rel_path_hash,
        BATCH_PARITY_CHUNKS as u64,
        cfg,
    )?;

    let mut count: u64 = 0;
    let mut batch_idx: u64 = 0;
    let file = fs::File::open(full_path)?;
    let mut reader = BufReader::with_capacity(FILE_READ_BUFFER, file);
    let mut packet_buf: Vec<u8> = Vec::with_capacity(HEADER_SIZE + CHUNK_SIZE);
    let mut batch_payloads: Vec<Vec<u8>> = Vec::with_capacity(BATCH_DATA_CHUNKS);

    loop {
        let mut chunk = vec![0u8; CHUNK_SIZE];
        let n = read_chunk(&mut reader, &mut chunk)?;
        if n == 0 {
            break;
        }
        chunk.truncate(n);
        send_data_packet(
            sock,
            target,
            &rel_path_hash,
            &chunk,
            count,
            &mut packet_buf,
            cfg,
        )?;
        batch_payloads.push(chunk);
        count += 1;
        if batch_payloads.len() == BATCH_DATA_CHUNKS {
            if let Err(e) = send_parity_packets(
                sock,
                target,
                &rel_path_hash,
                &batch_payloads,
                BATCH_PARITY_CHUNKS,
                batch_idx,
                &mut packet_buf,
                cfg,
            ) {
                error!(
                    "Failed to send parity for {} batch {}: {}",
                    rel_path, batch_idx, e
                );
            }
            batch_payloads.clear();
            batch_idx += 1;
        }
    }

    if !batch_payloads.is_empty() {
        if let Err(e) = send_parity_packets(
            sock,
            target,
            &rel_path_hash,
            &batch_payloads,
            BATCH_PARITY_CHUNKS,
            batch_idx,
            &mut packet_buf,
            cfg,
        ) {
            error!(
                "Failed to send parity for {} batch {}: {}",
                rel_path, batch_idx, e
            );
        }
    }

    send_end_packet(sock, target, full_path, rel_path, count, cfg)?;
    Ok(())
}

fn is_file_stable(path: &Path) -> bool {
    let initial = match fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    thread::sleep(STABILITY_CHECK_DELAY);
    match fs::metadata(path) {
        Ok(m) => m.len() == initial,
        Err(_) => false,
    }
}

fn walk_files(dir: &Path, results: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, results);
        } else if path.is_file() {
            results.push(path);
        }
    }
}

fn send_files(sock: &UdpSocket, target: &SocketAddr, source_dir: &Path, cfg: &SendCfg) {
    let mut files = Vec::new();
    walk_files(source_dir, &mut files);
    for full_path in files {
        let rel_path = match full_path.strip_prefix(source_dir) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel_path_str = match rel_path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if is_file_stable(&full_path) {
            if let Err(e) = send_file_chunks(sock, target, &full_path, &rel_path_str, cfg) {
                error!("Failed sending {}: {}", rel_path_str, e);
            }
        }
    }
}

fn spawn_udp_forwarder(args: &Args, sock: UdpSocket, target: SocketAddr, cfg: SendCfg) {
    let host = args.udp_host.clone();
    let port = args.udp_port.expect("udp_port must be set");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    thread::spawn(move || {
        let listen = match UdpSocket::bind((host.as_str(), port)) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to bind UDP listener {}:{} - {}", host, port, e);
                return;
            }
        };
        info!("Forwarding UDP packets arriving at {}:{}", host, port);
        let mut buf = vec![0u8; 65535];
        loop {
            match listen.recv_from(&mut buf) {
                Ok((n, _)) => {
                    let data = buf[..n].to_vec();
                    if tx.send(data).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("UDP recv error: {}", e);
                }
            }
        }
    });

    thread::spawn(move || {
        let mut packet_buf: Vec<u8> = Vec::with_capacity(HEADER_SIZE + CHUNK_SIZE);
        for data in rx {
            let pkg_id = {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                md5_hex(now.as_nanos().to_string().as_bytes())
            };
            let data_length = format!("{:032}", data.len());
            for _ in 0..UDP_RESEND {
                let mut count: u64 = 0;
                for chunk in data.chunks(CHUNK_SIZE) {
                    encode_packet_into(
                        &mut packet_buf,
                        PKG_TYPE_UDP,
                        count,
                        &data_length,
                        &pkg_id,
                        chunk,
                    );
                    if let Err(e) = send_to_blocking(&sock, &target, &packet_buf, &cfg) {
                        error!("UDP forward send error: {}", e);
                    }
                    count += 1;
                }
            }
        }
    });
}

fn tune_sndbuf(sock: &UdpSocket, want: i32) {
    let fd = sock.as_raw_fd();
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &want as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        warn!(
            "setsockopt(SO_SNDBUF, {}) failed: {}",
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
            libc::SO_SNDBUF,
            &mut got as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc == 0 {
        info!("SO_SNDBUF requested {} bytes, kernel granted {} bytes", want, got);
    } else {
        warn!(
            "getsockopt(SO_SNDBUF) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

fn resolve_target(addr: &str) -> std::io::Result<SocketAddr> {
    addr.to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no addresses"))
}

fn main() {
    let args = Args::parse();
    init_logger();

    let source_dir = PathBuf::from(&args.directory);
    if let Err(e) = fs::create_dir_all(&source_dir) {
        error!("Failed to create directory {}: {}", source_dir.display(), e);
        std::process::exit(1);
    }

    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to bind UDP socket: {}", e);
            std::process::exit(1);
        }
    };
    tune_sndbuf(&sock, args.sndbuf);
    if let Err(e) = sock.set_broadcast(true) {
        error!("Failed to enable broadcast: {}", e);
        std::process::exit(1);
    }

    let pacer = Arc::new(Pacer::from_bitrate_mbps(args.target_bitrate_mbps));
    if pacer.interval_nanos == 0 {
        info!("Pacing disabled (unlimited bitrate)");
    } else {
        info!(
            "Pacing: target {:.2} Mbps ({} ns per {}-byte packet)",
            args.target_bitrate_mbps, pacer.interval_nanos, PACER_PACKET_BYTES
        );
    }

    let cfg = SendCfg {
        pacer,
        max_backoff: Duration::from_millis(args.enobufs_max_backoff_ms),
    };

    let target_str = format!("{}:{}", args.target_subnet, args.target_port);
    let target = match resolve_target(&target_str) {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to resolve target address {}: {}", target_str, e);
            std::process::exit(1);
        }
    };

    if args.udp_port.is_some() {
        let fwd_sock = match sock.try_clone() {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to clone socket for UDP forwarder: {}", e);
                std::process::exit(1);
            }
        };
        spawn_udp_forwarder(&args, fwd_sock, target, cfg.clone());
    }

    info!("Watching for new files in: {}", source_dir.display());
    loop {
        send_files(&sock, &target, &source_dir, &cfg);
        thread::sleep(SCAN_INTERVAL);
    }
}
