# Setting up OpenBSD on Raspberry Pi 4B
## Prepare SD card and install USB drive
1. Got to the [download](https://www.openbsd.org/faq/faq4.html#Download) page and retrieve install installXX.img for arm64.
1. Flash the install firmware to a USB drive:
   ```
   sudo dd if=/home/user/Downloads/installXX.img of=/dev/sdb
   ```
1. Download a current version of the [Raspberry Pi 4 UEFI Firmware](https://github.com/pftf/RPi4/tags), then unzip the files:
   ```
   mkdir /tmp/rpi-firmware
   unzip Downloads/vXXX.zip -d /tmp/rpi-firmware
   ```
1. Create a boot partition with at least 20 MB, for example with `parted`:
   ```
   sudo parted /dev/mmcblk0
   (parted) mkpart
   Partition type?  primary/extended? primary
   File system type?  [ext2]? fat16
   Start? 1M
   End? 50M
   (parted) set 1 boot on
   (parted) quit
   ```
1. Mount the first partition of the SD card and copy the UEFI firmware:
   ```
   sudo mount /dev/mmcblk0p1 /mnt
   sudo cp /tmp/rpi-firmware/* /mnt/
   sudo umount /mnt
   ```
1. Plug the SD card and USB drive into the RPi.

## Configure UEFI and install OpenBSD
1. When the UEFI logo appears, hit the ESC key to enter the setup.
1. Use the boot options to boot the USB drive.  If problems occur, have a look at [AshyIsMe/openbsd-rpi4](https://github.com/AshyIsMe/openbsd-rpi4).
1. Run the installer and install OpenBSD to suite your needs. The default settings should be fine in most cases.
1. If you did overwrite the boot partition during installation, copy the files from `/tmp/rpi-firmware/` into the boot partition again.
1. Go to the UEFI "Boot Maintenance Manager" > "Boot Options" and create a new boot entry with the EFI file.
1. Change the boot order to boot the OpenBSD efi file first.

# Hardware setup
1. Attach one USB UART (a) adapter to sending RPi, and two USB UART (b & c) adapters to the receiving RPi.
1. Connect the ground of (a) to the ground of (b).
1. Connect Tx of (a) to Rx of (b).
1. Connect Ground of (c) to the ground of the Arduino, then connect Tx and Rx of (c) to Rx and Tx of the Arduino.
1. Flash the `arduino/arduino.ino` file on the Arduino.

# Diode setup
1. to be continued