# Data Diode - OpenCode Agent Instructions

## Project Overview

Rust-based data diode for OpenBSD/Raspberry Pi. Two binaries (`diode_send`, `diode_receive`) transfer files via serial/UDP through a unidirectional fiber optic link with hash verification.

## Package Structure

- **Monorepo with 2 Rust crates:**
  - `diode_send/src/main.rs` - sender daemon
  - `diode_receive/src/main.rs` - receiver daemon (with optional Arduino LCD support)
  - Root `Cargo.toml` defines workspace

## Developer Commands

```bash
# Build both binaries
cargo build --release

# Build receive binary with Arduino feature (for serial LCD support)
cargo build --release --features arduino --package diode-receive

# Run sender
cargo run --release --bin diode_send -- --directory /path/to/send --target-subnet 10.125.125.255 --target-port 5005

# Run receiver (without Arduino)
cargo run --release --bin diode_receive -- --directory /path/to/receive --bind-subnet 10.125.125.255 --bind-port 5005

# Run receiver (with Arduino LCD on /dev/cuaU0)
cargo run --release --bin diode_receive --features arduino -- --directory /path/to/receive --arduino /dev/cuaU0
```

## File Utilities (`bin/`)

- `bin/split_files <dir>` - Split files >10MB into chunks, creates SHA256 manifest
- `bin/merge_files <dir>` - Reassemble split files, verify against SHA256

These are used for large file transfers through the diode to minimize retransmission overhead.

## Deployment Architecture

**Sender (online network):**
- Installs `diode_send` binary to `/usr/local/bin/`
- Uses `etc/rc.d/diode_send` (OpenBSD) or `etc/systemd/diode-send.service` (Linux)
- Directory: `/home/diode/send`

**Receiver (air-gapped internal network):**
- Installs `diode_receive` binary to `/usr/local/bin/`
- Uses `etc/rc.d/diode_receive` (OpenBSD) or `etc/systemd/diode-receive.service` (Linux)
- Directory: `/home/diode/receive`
- Arduino optional: `--arduino /dev/cuaU0`

**Network:**
- Default broadcast subnet: `10.125.125.255`
- Default port: `5005`
- UDP forwarding: Sender listens on `--udp-port`, receiver forwards to `--udp-target-ip` and `--udp-target-port`

## Build Artifacts

- `target/` contains compiled binaries (checked into git for convenience)
- Binaries in `bin/` are pre-built releases

## Update Flow for OpenBSD Maintenance

When updating OpenBSD on the diode machines:

1. Always **update sender first** (`sysupgrade` + `pkg_add -u`)
2. Download new release files: `wget -nH -nc -r --no-parent https://cdn.openbsd.org/pub/OpenBSD/<VERSION>/arm64/`
3. Use `bin/split_files` on files >10MB before transfer
4. Transfer through diode
5. Use `bin/merge_files` on receiver
6. Update receiver packages (ensure Python dependencies transferred before upgrading receiver OS)
7. Run `sysupgrade` on receiver

Configuration file `etc/openbsd-mirror.conf` tracks which packages to download via `bin/download_packages` Python script.

## Key Technical Details

- **Chunk size:** 940 bytes, batched in groups of 100 packets
- **Resend logic:** Each batch sent twice to mitigate packet loss
- **Hash verification:** MD5 used for file integrity
- **Logging:** Dual console + syslog output

## Hardware Notes

- **USB Ethernet chipset:** ASIX AX88179 required for OpenBSD `axen0` driver
- **Arduino:** Optional LCD display on `/dev/cuaU0` (OpenBSD) or `/dev/ttyUSB0/USB1` (Linux)
- **USB ports:** Use USB 3 (blue) for best performance

## Known Issues

- After major OpenBSD version changes, `download_packages` may fail dependency resolution - clear config section and use `*` versioning
- `axen0` interface inactive: run `sh /etc/netstart /etc/hostname.axen0` or reboot

## Testing

### Automated loopback test: `./test_diode.sh`

Runs an end-to-end sender + receiver pair on `127.0.0.1:9999` against
freshly-generated 10 MB and 100 MB random files. The script:

1. Wipes `/tmp/test_send` and `/tmp/test_recv`.
2. Creates the two test files with `dd if=/dev/urandom`.
3. Records their MD5 sums.
4. Spawns `target/release/diode_receive` and `target/release/diode_send`.
5. Polls `/tmp/test_recv` for the final file names (the receiver writes to
   `<name>.partial` while batches stream in and only renames to the final
   name on END, so the polling does not race the in-flight transfer).
6. Kills both daemons, re-hashes the received files, and exits non-zero on
   any mismatch or timeout (300 s).

Requirements: a release build (`cargo build --release`). The script reads
the binaries straight from `target/release/`.

Use this as the default regression check after any change to the wire
format, sender, or receiver. For coverage of larger files, increase the
`bs`/`count` arguments to `dd` near the top of the script.

### Manual testing

When loopback is not sufficient (e.g., real serial/diode hardware):

1. Start receiver
2. Start sender
3. Drop test file in sender directory
4. Verify hash match and file appears in receiver directory
