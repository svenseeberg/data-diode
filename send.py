#!/usr/bin/env python3

import serial
from pathlib import Path
import os
import sys
import base64
import hashlib
import time

dev, baud = sys.argv[1].split(',')
with serial.Serial(dev, baud, timeout=1) as ser:
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
                ser.write(b'\n\r--filename--\n\r') # filename starts
                time.sleep(0.5)
                ser.write(filename)
                ser.write(b'\n\r--content---\n\r') # content starts
                time.sleep(0.5)
                with open(filepath, "rb") as fo:
                    while True:
                        content = fo.read(1024)
                        if not content:
                            break
                        content = base64.b64encode(content)
                        m.update(content)
                        ser.write(content)
                        ser.write(b'\n\r')
                ser.write(b'\n\r--hashsum---\n\r') # hash
                time.sleep(0.5)
                hashsum = m.hexdigest().encode("ascii","ignore")
                ser.write(hashsum)
                ser.write(b'\n\r--endfile---\n\r') # hash
                time.sleep(0.5)
                os.remove(filepath)
                print("Done.")
                time.sleep(1)
