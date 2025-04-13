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
   ```
1. Edit the `/mnt/config.txt` and disable the WIFI and Bluetooth adapters by adding the following lines:
   ```
   dtoverlay=disable-wifi
   dtoverlay=disable-bt
   ```
1. Unmount the SD card:
   ```
   sudo umount /mnt/
   ```
1. Plug the SD card and USB drive into the RPi.

# Hardware setup
1. Connect the fiber converters with USB ethernet adapters to the Raspberry Pis.
1. Use the fiber signal splitter to send the Tx signal from the sender back into the sender Rx and into the receiver Rx.
1. If you want to physically guarantee that there is no reverse channel, you can add a diode with a pull down or pull up resistor betwen the Tx and Rx pins. Alternatively, an optocoupler can be used to galvanically separate the circuits. Don't forget to [disconnect the bluetooth and WLAN antennae](https://raspberrypi.stackexchange.com/questions/114596/desolder-wifi-bluetooth-module-on-a-raspberry-pi-4).
1. Flash the `arduino/arduino.ino` file on the Arduino.
1. Connect the Arduino via USB to the receiving RPi.
1. Set up the power supply for both RPis.
1. Attach one USB Ethernet adapters to each Raspberry Pi. Use the (blue) USB 3 ports.

## Configure UEFI and install OpenBSD
1. When the UEFI logo appears, hit the ESC key to enter the setup.
1. Use the boot options to boot the USB drive. If problems occur, have a look at [AshyIsMe/openbsd-rpi4](https://github.com/AshyIsMe/openbsd-rpi4).
1. Run the installer and install OpenBSD to suite your needs. The default settings should be fine in most cases.
1. If you did overwrite the boot partition during installation, copy the files from `/tmp/rpi-firmware/` into the boot partition again. Don't forget to edit the `config.txt`.
1. Go to the UEFI "Boot Maintenance Manager" > "Boot Options" and create a new boot entry with the EFI file.
1. Change the boot order to boot the OpenBSD efi file first.
1. Boot and log in as root.
1. Configure the additional USB network [interface](https://man.openbsd.org/hostname.if.5). Assign `10.125.125.1/24` on the sender and `10.125.125.2/24` on the receiver.

# Diode setup
Clone this repo or download the latest .zip file and extract. Then `cd` into the directory.
## Receiver setup
1. Install Python3.
1. Copy the rc.d file:
   ```
   cp ./etc/rc.d/diode_receive /etc/rc.d/
   ```
1. Edit the device paths in `/etc/rc.d/diode_receive`. If you do not have an Arduino with LCD display connected, remove the `--arduino` parameter.
1. Create the directory to which the received files are written.
1. Copy the main program:
   ```
   cp ./bin/diode_receive /usr/local/bin/
   ```
1. Enable the service:
   ```
   rcctl enable diode_receive
   rcctl start diode_receive
   ```

## Sender setup
1. Install Python3
1. Copy the rc.d file:
   ```
   cp ./etc/rc.d/diode_send /etc/rc.d/
   ```
1. Edit the device paths in `/etc/rc.d/diode_send`.
1. Create the directory from wich the files are read.
1. Copy the main program:
   ```
   cp ./bin/diode_send /usr/local/bin/
   ```
1. Enable the service:
   ```
   rcctl enable diode_send
   rcctl start diode_send
   ```

## Adjust transfer speeds
Transfer some files. If not all they are not appearing on the receiver, there were transmission errors. Reduce the bit rate in the rc.d scripts until you have no transmission errors. Alternatively, improve your hardware setup.
