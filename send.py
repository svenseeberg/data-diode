#!/usr/bin/env python3

import serial
from pathlib import Path
import os
import sys
import base64
import hashlib
import time

with serial.Serial(sys.argv[1], 19200, timeout=1) as ser:
    def wln(line):
        ser.write(line)
        ser.write(b'\n')
    read_dir = os.path.join(os.environ['HOME'], 'diode-send')
    print("Scanning %s" % read_dir)
    while True:
        for root, dirs, files in os.walk(read_dir):
            for filename in files:
                print("Sending %s" % filename)
                filepath = os.path.join(os.environ['HOME'], 'diode-send', filename)
                m = hashlib.md5()
                filename = base64.b64encode(bytes(filename, 'ascii'))
                m.update(filename)
                wln(bytes(chr(1), 'ascii')) # SOH - Start Of Header
                wln(filename)
                wln(bytes(chr(2), 'ascii')) # STX - Start of Text
                with open(filepath, "rb") as fo:
                    while True:
                        content = fo.read(1024)
                        if not content:
                            break
                        content = base64.b64encode(content)
                        m.update(content)
                        wln(content)
                wln(bytes(chr(3), 'ascii')) # ETX - End of Text
                time.sleep(0.5)
                hashsum = m.hexdigest().encode("ascii","ignore")
                wln(hashsum)
                wln(bytes(chr(4), 'ascii')) # EOT - End of Transmission
                time.sleep(0.5)
                os.remove(filepath)
                print("Done.")
                time.sleep(1)
