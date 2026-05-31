use md5::{Digest, Md5};

pub fn md5_hex(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(32);
    for byte in result.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}
