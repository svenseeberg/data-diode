//! Wire-format constants and helpers shared between the sender and receiver.

use std::io::Write;

/// Maximum data payload per packet (before FEC padding).
pub const CHUNK_SIZE: usize = 940;

/// FEC shard size. Reed-Solomon-SIMD requires a multiple of 64; we round
/// CHUNK_SIZE up so data chunks can be zero-padded into a valid shard while
/// keeping wire packets at most CHUNK_SIZE for normal data packets.
pub const SHARD_SIZE: usize = 960;

/// Size of the fixed header preceding every packet payload.
pub const HEADER_SIZE: usize = 1 + 10 + 32 + 32;

/// Number of data chunks per FEC batch. The sender accumulates this many data
/// packets, emits them, then computes [`BATCH_PARITY_CHUNKS`] parity shards
/// over the batch before moving on. The receiver finalises a batch (writing
/// its bytes to disk) as soon as the next batch begins, so neither side has
/// to hold a full file in memory.
pub const BATCH_DATA_CHUNKS: usize = 1000;

/// Number of FEC parity shards generated per batch.
pub const BATCH_PARITY_CHUNKS: usize = 100;

pub const PKG_TYPE_START: u8 = 0;
pub const PKG_TYPE_DATA: u8 = 1;
pub const PKG_TYPE_END: u8 = 2;
pub const PKG_TYPE_PARITY: u8 = 3;
pub const PKG_TYPE_UDP: u8 = 4;

/// Sentinel value used wherever a hash field is unused.
pub const EMPTY_HASH: &str = "00000000000000000000000000000000";

/// Encode a packet into the supplied buffer, replacing its current contents.
/// `count_or_filesize` is zero-padded to 10 ASCII digits. Hash fields must
/// each be exactly 32 bytes (typically MD5 hex strings or `EMPTY_HASH`).
pub fn encode_packet_into(
    buf: &mut Vec<u8>,
    pkg_type: u8,
    count_or_filesize: u64,
    path_hash: &str,
    data_hash: &str,
    payload: &[u8],
) {
    debug_assert_eq!(path_hash.len(), 32);
    debug_assert_eq!(data_hash.len(), 32);
    buf.clear();
    buf.reserve(HEADER_SIZE + payload.len());
    let _ = write!(buf, "{}{:010}", pkg_type, count_or_filesize);
    buf.extend_from_slice(path_hash.as_bytes());
    buf.extend_from_slice(data_hash.as_bytes());
    buf.extend_from_slice(payload);
}

/// Encode a packet with the standard header followed by `payload`.
/// See [`encode_packet_into`] for the in-place variant used on hot paths.
pub fn encode_packet(
    pkg_type: u8,
    count_or_filesize: u64,
    path_hash: &str,
    data_hash: &str,
    payload: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
    encode_packet_into(&mut buf, pkg_type, count_or_filesize, path_hash, data_hash, payload);
    buf
}

/// Encode the parity-count value carried in the data-hash field of START
/// packets. `parity_count == 0` produces the same string as `EMPTY_HASH`,
/// so a sender that disables FEC is indistinguishable from the legacy
/// "no FEC" wire format.
pub fn encode_parity_field(parity_count: u64) -> String {
    format!("{:032}", parity_count)
}

/// Inverse of [`encode_parity_field`]. Returns `None` if the field is not a
/// valid ASCII decimal integer.
pub fn decode_parity_field(bytes: &[u8]) -> Option<u64> {
    std::str::from_utf8(bytes).ok()?.parse::<u64>().ok()
}
