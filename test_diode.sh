#!/bin/bash
set -e

echo "=== Data Diode Test Script ==="
echo ""

# Setup test directories
rm -rf /tmp/test_send /tmp/test_recv
mkdir -p /tmp/test_send /tmp/test_recv

# Create test files
echo "Creating test files..."
dd if=/dev/urandom of=/tmp/test_send/file_10mb.bin bs=1M count=10 2>/dev/null
dd if=/dev/urandom of=/tmp/test_send/file_100mb.bin bs=1M count=100 2>/dev/null

# Calculate original MD5 sums
echo ""
echo "Original MD5 sums:"
MD5_10MB_ORIG=$(md5sum /tmp/test_send/file_10mb.bin | awk '{print $1}')
MD5_100MB_ORIG=$(md5sum /tmp/test_send/file_100mb.bin | awk '{print $1}')
echo "  10MB file: $MD5_10MB_ORIG"
echo "  100MB file: $MD5_100MB_ORIG"
echo ""

# Start receiver
echo "Starting receiver..."
/home/opencode/projects/data-diode/target/release/diode_receive \
    --directory /tmp/test_recv \
    --bind-subnet 127.0.0.1 \
    --bind-port 9999 &
RECV_PID=$!
sleep 2

# Start sender
echo "Starting sender..."
/home/opencode/projects/data-diode/target/release/diode_send \
    --directory /tmp/test_send \
    --target-subnet 127.0.0.1 \
    --target-port 9999 &
SEND_PID=$!

# Wait for transfer (allow up to 5 minutes)
echo ""
echo "Waiting for transfer to complete..."
TIMEOUT=300
START_TIME=$SECONDS
while true; do
    ELAPSED=$((SECONDS - START_TIME))
    if [ $ELAPSED -gt $TIMEOUT ]; then
        echo "ERROR: Timeout after ${TIMEOUT}s"
        break
    fi
    
    # Check if all expected files arrived
    if [ -f /tmp/test_recv/file_10mb.bin ] && [ -f /tmp/test_recv/file_100mb.bin ]; then
        echo "All files received!"
        break
    fi
    
    sleep 1
done

# Stop sender and receiver
kill $SEND_PID $RECV_PID 2>/dev/null
wait 2>/dev/null
sleep 2

# Calculate received MD5 sums and compare
echo ""
echo "Received file MD5 sums:"
if [ -f /tmp/test_recv/file_10mb.bin ]; then
    MD5_10MB_RECV=$(md5sum /tmp/test_recv/file_10mb.bin | awk '{print $1}')
    echo "  10MB file: $MD5_10MB_RECV"
    if [ "$MD5_10MB_ORIG" = "$MD5_10MB_RECV" ]; then
        echo "  ✓ 10MB file: MATCH"
    else
        echo "  ✗ 10MB file: MISMATCH!"
        exit 1
    fi
else
    echo "  ✗ 10MB file: MISSING"
    exit 1
fi

if [ -f /tmp/test_recv/file_100mb.bin ]; then
    MD5_100MB_RECV=$(md5sum /tmp/test_recv/file_100mb.bin | awk '{print $1}')
    echo "  100MB file: $MD5_100MB_RECV"
    if [ "$MD5_100MB_ORIG" = "$MD5_100MB_RECV" ]; then
        echo "  ✓ 100MB file: MATCH"
    else
        echo "  ✗ 100MB file: MISMATCH!"
        exit 1
    fi
else
    echo "  ✗ 100MB file: MISSING"
    exit 1
fi

echo ""
echo "=== All tests passed! ==="

# Cleanup
rm -rf /tmp/test_send /tmp/test_recv
