## About
This project contains the source code for a DIY data diode. It uses
Raspberry Pis to unidirectionally transmit data via the serial
interface from one to the other. As the reverse direction pins are
not connected, no transfer in the other direction is possible. An
Arduino can be used to monitor the traffic and show the status on
a 1602 LCD.

![Finished Diode](images/case.jpg)

The software is primarily developed for OpenBSD but will also work
on Raspbian or Debian. OpenBSD seems better suited as it is easier
to maintain a mirror repository of the core operating system and
selected packages. A simple script to download the required files
and copy them through the diode is included.

## How it works
The sending Raspberry Pi continuously checks a directory for new files.
Files can be dropped into this directory with any protocol. If a new
file is detected, it will be split into chunks, which will then be
transferred through the unidirectional serial wire. In the end, a hash
sum is transferred as well. If the hash of the transferred data matches
the sent hash, the received file will be stored in a target directory of
the receiving Raspberry Pi, ready for pick up. If the hashes do not
match, the error counter on the display increases by one.

The display shows the status of the diode (idle/transfer in progress),
the total number of files transferred, the number of errors that
occured, the total amount of transferred KB, and the progress
(percentage) of the current file transfer.

## Speed
The speed of the diode is mostly limited by the UART devices. With cheap
USB UART adapters, a data rate of about 20KB/s can be achieved. This is
fast enough to keep a mirror of OpenBSD with a selected subset of
packages up to date in an internal network.

## Software Installation
For details about the installation, read [INSTALL.md](INSTALL.md).

## Required Hardware
* 2x Raspberry Pi 4B
* 2x USB UART serial adapters
## Optional Hardware
* 1x Arduino
* 1x 1602 LCD with I2C
* 1x Diode
* 1x USB power supply with 3 outlets
* 1x large enough case to house everything
* 2x RJ45 feedthroughs (i.e. Neutrik NE8FDP)

![images/inside](images/inside.jpg)
