#!/bin/bash
# End-to-end loopback regression test for the data diode.
#
# Part 1 pushes a 10 MB and a 100 MB random file through a sender/receiver
# pair on 127.0.0.1 and compares MD5 sums.
#
# Part 2 exercises the UDP forwarding path (--udp-port on the sender,
# --udp-target-port on the receiver) with netcat: a datagram is sent into the
# sender's UDP listener, travels the diode as PKG_TYPE_UDP fragments, and is
# reassembled by the receiver and forwarded to an nc listener that captures it.
#
# Part 3 exercises the split_files / merge_files helpers (bin/split_files and
# bin/merge_files): a second 100 MB random file is split into 10 chunks, then
# merged back and compared byte-for-byte to the original, and the chunks must
# be cleaned up on a successful digest match.
#
# Requires a release build: cargo build --release
set -e

# Resolve the repo root from the script's own location so the test runs
# independently of the developer's working checkout path (CI, other machines,
# etc.). BIN_DIR still wins if explicitly set (used for overriding to point at
# a specific target/ tree, e.g. a cross-compiled release layout).
BIN_DIR=${BIN_DIR:-$(cd "$(dirname "$0")" && pwd)}
RECV_BIN=$BIN_DIR/target/release/diode_receive
SEND_BIN=$BIN_DIR/target/release/diode_send
SPLIT_BIN=$BIN_DIR/bin/split_files
MERGE_BIN=$BIN_DIR/bin/merge_files

DIODE_BIND_PORT=9999   # the diode "wire" (sender -> receiver)
SENDER_UDP_PORT=12345  # sender's --udp-port (where the test nc sends)
NC_LISTEN_BASE=12680   # receiver's --udp-target-port, +1 per scenario

# The sender repeats every forwarded datagram this many times for resilience
# (UDP_RESEND in diode_send/src/main.rs). The receiver has no dedup, so each
# datagram legitimately arrives at the capture listener this many times.
UDP_RESEND=2

# `nc -u` reads stdin in blocks of this size and emits one datagram per block,
# so a payload larger than this arrives as several datagrams. Verified
# empirically against the OpenBSD nc in Debian. Keep single-datagram payloads
# at or below this value.
NC_WRITE_SIZE=16384

# Seconds to wait for a forwarded capture to reach its expected size.
FORWARD_GRACE=8

SEND_PID=""
RECV_PID=""
NC_PID=""

echo "=== Data Diode Test Script ==="
echo ""

for b in "$RECV_BIN" "$SEND_BIN"; do
    if [ ! -x "$b" ]; then
        echo "ERROR: $b missing. Run: cargo build --release" 1>&2
        exit 1
    fi
done

for b in "$SPLIT_BIN" "$MERGE_BIN"; do
    if [ ! -x "$b" ]; then
        echo "ERROR: $b missing. Expected in $BIN_DIR/bin/" 1>&2
        exit 1
    fi
done

cleanup() {
    [ -n "$SEND_PID" ] && kill "$SEND_PID" 2>/dev/null || true
    [ -n "$RECV_PID" ] && kill "$RECV_PID" 2>/dev/null || true
    [ -n "$NC_PID" ] && kill "$NC_PID" 2>/dev/null || true
    # Only ever matches binaries from this checkout.
    pkill -f "$BIN_DIR/target/release/diode_" 2>/dev/null || true
    wait 2>/dev/null || true
}
trap cleanup EXIT

rm -rf /tmp/test_send /tmp/test_recv /tmp/test_udp /tmp/test_split
mkdir -p /tmp/test_send /tmp/test_recv /tmp/test_udp /tmp/test_split

# ---------------------------------------------------------------------------
# Part 1: file transfer
# ---------------------------------------------------------------------------

echo "Creating test files..."
dd if=/dev/urandom of=/tmp/test_send/file_10mb.bin bs=1M count=10 2>/dev/null
dd if=/dev/urandom of=/tmp/test_send/file_100mb.bin bs=1M count=100 2>/dev/null

echo ""
echo "Original MD5 sums:"
MD5_10MB_ORIG=$(md5sum /tmp/test_send/file_10mb.bin | awk '{print $1}')
MD5_100MB_ORIG=$(md5sum /tmp/test_send/file_100mb.bin | awk '{print $1}')
echo "  10MB file:  $MD5_10MB_ORIG"
echo "  100MB file: $MD5_100MB_ORIG"
echo ""

echo "Starting receiver..."
"$RECV_BIN" \
    --directory /tmp/test_recv \
    --bind-subnet 127.0.0.1 \
    --bind-port "$DIODE_BIND_PORT" \
    >/tmp/test_udp/file_recv.log 2>&1 &
RECV_PID=$!
sleep 2

echo "Starting sender..."
"$SEND_BIN" \
    --directory /tmp/test_send \
    --target-subnet 127.0.0.1 \
    --target-port "$DIODE_BIND_PORT" \
    >/tmp/test_udp/file_send.log 2>&1 &
SEND_PID=$!

echo ""
echo "Waiting for transfer to complete..."
TIMEOUT=300
START_TIME=$SECONDS
while true; do
    if [ $((SECONDS - START_TIME)) -gt $TIMEOUT ]; then
        echo "ERROR: Timeout after ${TIMEOUT}s"
        break
    fi
    if [ -f /tmp/test_recv/file_10mb.bin ] && [ -f /tmp/test_recv/file_100mb.bin ]; then
        echo "All files received!"
        break
    fi
    sleep 1
done

kill "$SEND_PID" "$RECV_PID" 2>/dev/null || true
SEND_PID=""; RECV_PID=""
wait 2>/dev/null || true
sleep 2

echo ""
echo "Received file MD5 sums:"
check_file() {
    local label=$1 path=$2 want=$3 got
    if [ ! -f "$path" ]; then
        echo "  x $label: MISSING"
        return 1
    fi
    got=$(md5sum "$path" | awk '{print $1}')
    echo "  $label: $got"
    if [ "$want" = "$got" ]; then
        echo "  + $label: MATCH"
    else
        echo "  x $label: MISMATCH!"
        return 1
    fi
}
check_file "10MB file " /tmp/test_recv/file_10mb.bin "$MD5_10MB_ORIG"
check_file "100MB file" /tmp/test_recv/file_100mb.bin "$MD5_100MB_ORIG"

# ---------------------------------------------------------------------------
# Part 2: UDP forwarding
# ---------------------------------------------------------------------------

# Start a sender/receiver pair whose UDP forwarding targets $1. The sender's
# --directory is empty by now (diode_send removes each file once its END
# packet is out), so the pair is dedicated to the UDP path.
start_pair() {
    local target_port=$1
    "$RECV_BIN" \
        --directory /tmp/test_recv \
        --bind-subnet 127.0.0.1 \
        --bind-port "$DIODE_BIND_PORT" \
        --udp-target-ip 127.0.0.1 \
        --udp-target-port "$target_port" \
        >/tmp/test_udp/receive.log 2>&1 &
    RECV_PID=$!
    "$SEND_BIN" \
        --directory /tmp/test_send \
        --target-subnet 127.0.0.1 \
        --target-port "$DIODE_BIND_PORT" \
        --udp-port "$SENDER_UDP_PORT" \
        >/tmp/test_udp/send.log 2>&1 &
    SEND_PID=$!
    sleep 2
}

stop_pair() {
    [ -n "$SEND_PID" ] && kill "$SEND_PID" 2>/dev/null || true
    [ -n "$RECV_PID" ] && kill "$RECV_PID" 2>/dev/null || true
    SEND_PID=""; RECV_PID=""
    wait 2>/dev/null || true
    sleep 1
}

# Capture every datagram arriving on port $2 into file $1, in arrival order.
start_nc_listener() {
    local outfile=$1 port=$2
    : > "$outfile"
    nc -u -l -p "$port" >"$outfile" 2>/dev/null </dev/null &
    NC_PID=$!
    sleep 0.5
    if ! kill -0 "$NC_PID" 2>/dev/null; then
        NC_PID=""
        return 1
    fi
}

stop_nc_listener() {
    [ -n "$NC_PID" ] && kill "$NC_PID" 2>/dev/null || true
    NC_PID=""
    sleep 0.3
}

# Write to $2 the exact byte stream the capture listener must see for the
# payload in $1: nc splits the payload into NC_WRITE_SIZE-byte datagrams and
# the diode forwards each one UDP_RESEND times, in order.
expected_capture() {
    local src=$1 out=$2 size blocks b i
    size=$(wc -c < "$src")
    blocks=$(( (size + NC_WRITE_SIZE - 1) / NC_WRITE_SIZE ))
    : > "$out"
    b=0
    while [ "$b" -lt "$blocks" ]; do
        i=0
        while [ "$i" -lt "$UDP_RESEND" ]; do
            dd if="$src" bs="$NC_WRITE_SIZE" skip="$b" count=1 status=none >> "$out"
            i=$((i + 1))
        done
        b=$((b + 1))
    done
}

# run_udp_scenario <name> <port-offset> <payload-size>...
#
# Creates one random payload per size, sends each as its own `nc -u`
# invocation, and requires the capture to equal the concatenation of the
# per-payload expectations byte for byte. Each scenario gets a fresh
# sender/receiver pair (so the receiver's reassembly state starts clean) and
# its own listener port (so no cross-scenario bleed is possible).
run_udp_scenario() {
    local name=$1 offset=$2
    shift 2
    local tport=$((NC_LISTEN_BASE + offset))
    local out=/tmp/test_udp/listen_${name}.bin
    local exp=/tmp/test_udp/expect_${name}.bin
    local srcs=() sizes="$*"
    local n=0 sz s part want got i

    : > "$exp"
    for sz in "$@"; do
        s=/tmp/test_udp/src_${name}_${n}.bin
        part=/tmp/test_udp/expect_${name}_${n}.bin
        head -c "$sz" /dev/urandom > "$s"
        expected_capture "$s" "$part"
        cat "$part" >> "$exp"
        srcs+=("$s")
        n=$((n + 1))
    done
    want=$(wc -c < "$exp")

    echo ""
    echo "Scenario $name: payload bytes [$sizes] -> 127.0.0.1:$tport"
    if ! start_nc_listener "$out" "$tport"; then
        echo "  x $name: nc listener failed to start on port $tport"
        return 1
    fi
    start_pair "$tport"

    for s in "${srcs[@]}"; do
        nc -u -w 2 127.0.0.1 "$SENDER_UDP_PORT" < "$s" || true
        sleep 0.5
    done

    i=0
    while [ "$i" -lt "$FORWARD_GRACE" ]; do
        got=$(wc -c < "$out")
        if [ "$got" -ge "$want" ]; then
            break
        fi
        sleep 1
        i=$((i + 1))
    done
    sleep 1
    stop_nc_listener
    stop_pair

    if cmp -s "$exp" "$out"; then
        echo "  + $name: $(wc -c < "$out") B captured, byte-exact" \
             "(each datagram forwarded ${UDP_RESEND}x)"
        return 0
    fi

    echo "  x $name: capture does not match expectation"
    echo "      expected $want B, captured $(wc -c < "$out") B"
    cmp "$exp" "$out" 2>&1 | head -3 | sed 's/^/      /'
    echo "      receiver log:"
    tail -6 /tmp/test_udp/receive.log | sed 's/^/        /'
    echo "      sender log:"
    tail -6 /tmp/test_udp/send.log | sed 's/^/        /'
    return 1
}

echo ""
echo "=== UDP forwarding scenarios ==="

# One fragment: completes on the count==0 packet alone.
run_udp_scenario udp_single_fragment 0 100

# Three fragments (940 + 940 + 120): requires the receiver to accumulate
# across continuation packets. This is the case that regressed when drain()
# cleared the reassembly buffer after every fragment.
run_udp_scenario udp_three_fragments 1 2000

# 18 fragments in a single datagram, just under nc's per-write limit.
run_udp_scenario udp_many_fragments 2 16000

# Two datagrams back to back: the second flow's count==0 must take over
# cleanly once the first has been forwarded, with no bleed between them.
run_udp_scenario udp_two_datagrams 3 500 5000

# A payload past nc's write size arrives as four datagrams of 18 fragments
# each: four consecutive multi-fragment flows through one receiver.
run_udp_scenario udp_datagram_stream 4 65000

# ---------------------------------------------------------------------------
# Part 3: split_files / merge_files
# ---------------------------------------------------------------------------
#
# Exercises the file-splitting helpers used in the OpenBSD update flow: a
# second 100 MB random file is split into fixed-size chunks (>10 MB each, so
# split actually fires), then merged back and compared byte-for-byte to the
# original. split_files writes a SHA256 manifest and merge_files trusts it, so
# a corrupt merge (a bad glob, dropped chunk, or reordered chunk) is caught by
# the digest and the chunks are retained on failure.
#
# 100 MB split at the 10 MB threshold = 10 chunks.

echo ""
echo "=== split_files / merge_files ==="

echo "Creating second 100 MB test file..."
dd if=/dev/urandom of=/tmp/test_split/file_100mb.bin bs=1M count=100 2>/dev/null
MD5_SPLIT_ORIG=$(md5sum /tmp/test_split/file_100mb.bin | awk '{print $1}')
echo "  original: $MD5_SPLIT_ORIG"

# split_files is POSIX-sh; run it explicitly under /bin/sh so the test does
# not depend on this shell's glob / word-splitting semantics.
if ! /bin/sh "$SPLIT_BIN" /tmp/test_split; then
    echo "  x split_files: returned non-zero"
    exit 1
fi

# After a correct split: the 10 MB chunks exist, the original is gone, and a
# SHA256 manifest is present. `find -name` is a substring match, so
# file_100mb.bin_chunk_aa matches.
CHUNKS_LEFT=$(find /tmp/test_split -type f -name 'file_100mb.bin_chunk_*' | wc -l | awk '{print $1}')
echo "  chunks generated: $CHUNKS_LEFT"
if [ ! -f /tmp/test_split/SHA256 ]; then
    echo "  x split_files: manifest missing"
    exit 1
fi
if [ -f /tmp/test_split/file_100mb.bin ]; then
    echo "  x split_files: original should have been removed by split"
    exit 1
fi
if [ "$CHUNKS_LEFT" -ne 10 ]; then
    echo "  x split_files: expected 10 chunks, found $CHUNKS_LEFT"
    exit 1
fi
echo "  + split_files: 10 chunks, manifest present, original removed"

# Merging must restore the original file byte-for-byte. merge_files recomputes
# sha256 and compares against the manifest, so a bad reassembly fails here.
if ! /bin/sh "$MERGE_BIN" /tmp/test_split; then
    echo "  x merge_files: returned non-zero"
    exit 1
fi
MD5_SPLIT_MERGED=$(md5sum /tmp/test_split/file_100mb.bin | awk '{print $1}')
echo "  merged:   $MD5_SPLIT_MERGED"
if [ "$MD5_SPLIT_ORIG" != "$MD5_SPLIT_MERGED" ]; then
    echo "  x merge_files: merge does not match original"
    exit 1
fi
echo "  + merge_files: md5 match"

# On a digest match merge_files removes the chunks; none should remain.
CHUNKS_LEFT=$(find /tmp/test_split -type f -name 'file_100mb.bin_chunk_*' | wc -l | awk '{print $1}')
if [ "$CHUNKS_LEFT" -ne 0 ]; then
    echo "  x merge_files: $CHUNKS_LEFT chunks retained after success"
    exit 1
fi
echo "  + merge_files: chunks cleaned up"

echo ""
echo "=== All tests passed! ==="

rm -rf /tmp/test_send /tmp/test_recv /tmp/test_udp /tmp/test_split
