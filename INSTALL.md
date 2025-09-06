# Setting up OpenBSD on Raspberry Pi 4B
## Prepare USB drive with OpenBSD installer
1. Got to the [download](https://www.openbsd.org/faq/faq4.html#Download) page and retrieve install installXX.img for arm64.
1. Flash the install firmware to a USB drive:
   ```
   sudo dd if=/home/user/Downloads/installXX.img of=/dev/sdb
   ```
## Prepare SD card with UEFI firmware
1. Download a current version of the [Raspberry Pi 4 UEFI Firmware](https://github.com/pftf/RPi4/tags), then unzip the files:
   ```
   mkdir /tmp/rpi-firmware
   cd /tmp/rpi-firmware
   wget https://github.com/pftf/RPi4/releases/download/v1.41/RPi4_UEFI_Firmware_v1.41.zip
   unzip RPi4_UEFI_Firmware_v1.41.zip
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
   sudo cp /tmp/rpi-firmware/RPi4_UEFI_Firmware_v1.41/* /mnt/
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
1. Plug the SD card and USB drive into the RPi and power up.
1. Use the ESC key to open the UEFI Firmware and use the Boot Manager to boot the installer from the USB drive.
1. Install OpenBSD.

# Hardware setup
1. Attach one USB Ethernet adapters to each Raspberry Pi. Use the (blue) USB 3 ports.
1. Connect the USB Ethernet adapters to the fiber converters.
1. Use the fiber signal splitter to send the Tx signal from the sender back into the sender Rx and into the receiver Rx.
1. Set up the power supply for both RPis.
1. Optional: Flash the `arduino/arduino.ino` file on the Arduino.
1. Optional: Connect the Arduino via USB to the receiving RPi.

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
1. Create a user `diode`.
1. Copy the rc.d file:
   ```
   cp ./etc/rc.d/diode_receive /etc/rc.d/
   ```
1. Optional: if you're using an Arduino with display, allow the `diode` user to write to the serial port:
   ```sh
   usermod -G dialer diode
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
1. Install Python3.
1. Create a user `diode`:
   ```sh
   adduser diode
   ```
1. Copy the rc.d file:
   ```
   cp ./etc/rc.d/diode_send /etc/rc.d/
   ```
1. Edit the device paths in `/etc/rc.d/diode_send`.
1. Optional: if you're not using an Arduino, remove the `--arduino` argument in the `/etc/rc.d/diode_`
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

## Set Up OpenBSD Mirror on Receiver

1. To serve received files in the internal network via HTTP, edit the `/etc/httpd.conf` on your receiver Pi:

   ```
   chroot /home/diode/receive

   server "default" {
       listen on * port 80
       directory auto index
       root "/www"
   }
   ```

1. Start the daemon:

   ```sh
   rcctl start httpd
   rcctl enable httpd
   ```

   Now, if you copy files into the `/home/diode/send/www` directory on the sender, the files will be available via HTTP in the internal network.

1. You can now edit the `/etc/installurl` file on all your machines in the internal network and set the path to the IP adress of your receiver Pi:

   ```
   http://10.0.0.1/pub/OpenBSD
   ```

The [UPDATE.md](UPDATE.md) describes how to download and transfer the necessary files for updating the base system and packages.
