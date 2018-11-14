#!/usr/bin/env python3

import serial
from pathlib import Path
import os
import sys
import base64
import hashlib

write_dir = os.path.join(os.environ['HOME'], 'diode-receive')

with serial.Serial(sys.argv[1], 57600) as ser:
    m = hashlib.md5()
    started = False
    while True:
        line = ser.readline().strip().decode('ascii')
        if(line == chr(1)): # SOH - Start Of Header
            started = True
            filename_b64 = ser.readline().strip()
            filename = base64.b64decode(filename_b64).decode('ascii')
            m.update(filename_b64)
            print("Receiving file %s" % filename)
        elif(started == True and line == chr(2)): # STX - Start of Text
            print("Buffering content.")
            content = b''
            n = 0
            while True:
                line = ser.readline().strip()
                if(line == bytes(chr(3), 'ascii')): # ETX - End of Text:
                    hashsum = ser.readline().strip()
                    break
                else:
                    try:
                        n = n + 1
                        m.update(line)
                        content += base64.b64decode(line)
                    except:
                        break
        elif(started == True and line == chr(4)): # EOT - End of Transmission
            hashsum_received = m.hexdigest().encode("ascii","ignore")
            if(hashsum_received == hashsum):
                filepath = os.path.join(write_dir, filename)
                print("Hashsums match, writing to %s" % filepath)
                f = open(filepath, "wb")
                f.write(content)
                f.close()
                print("Done.")
            else:
                print("Hashsums do not match.")
            content = None
            filename = None
            content_b64 = None
            filename_b64 = None
            hashsum = None
            hashsum_received = None
            m = hashlib.md5()
            started = False
        else:
            pass
