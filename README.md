# About
This project contains the source code for a DIY data diode. It uses
Raspberry Pis to transmit data via the serial interface from one to
the other. An Arduino device can be used to monitor the traffic and
show the status on an 1602 LCD.

# Installation
1) Install any OS that supports Python3 on the Raspberry Pis
2) Move the scripts in the ```bin``` directory to ```/usr/bin/```
3) Move the service files from ```systemd``` to
   ```/usr/share/systemd/system```
4) Flash the SerialMonitor.ino to an Arduino.
5) Connect the Grounds of both Raspberry Pis and the Arduino
6) Connect the Tx of one Raspberry Pi with the Rx of the other Pi and
   the Arduino. If required, place a diode between the Tx and the Rx
   pins.
7) Reload the systemd configuration on the Raspberry Pis with
   ```systemctl daemon-reload```. Then start the service with
   ```systemctl enable diode-send@/dev/ttyS0``` and
   ```systemctl enable diode-receive@/dev/ttyS0```.
8) On both Pis start ```raspi-config``` and disable boot output on the
   Serial ports.
9) Reboot the Raspberry Pis

# Required Parts
* 2x Raspberry Pi
* 1x Arduino
* 1x 1602 LCD with I2C
* 1x Diode
