# Updating OpenBSD

This document describes how to maintain OpenBSD on the diode Raspberry Pis (and other OpenBSD machines in the internal network).

Warning: Updating OpenBSD can break the diode if the installed Python3 packages are not updated along with the main release.

## Upgrade Sender

To update OpenBSD on the diode, first update the sender. This allows updating all packages while being connected to the internet and problems can be dealt with much easier.

1. First upgrade to the newest OpenBSD release:
   ```sh
   sysupgrade
   ```
1. When the system is available again, upgrade the installed packages:
   ```sh
   pkg_add -u
   ```
1. Check if the sender scripts works as expected:
   ```sh
   rcctl check diode_send
   ```


## Transfer Files through Diode

When the upgrade of ther sender works as expected, all required files can be transmitted through the diode. When the transfer is completed, the receive can be updated as well.

1. Download the tgz files for updating OpenBSD:
   ```sh
   BSD_VERSION=7.5
   wget -nH -nc -r --no-parent https://cdn.openbsd.org/pub/OpenBSD/$BSD_VERSION/arm64/ -R "index.html*" --reject iso,img
   ```
1. Optional: split files larger than 10MB into chunks:
   ```sh
   split_files pub/OpenBSD/$BSD_VERSION
   ```
1. Copy files to diode directory:
   ```sh
   cp -r pub/OpenBSD/$BSD_VERSION /home/diode/send/www/pub/OpenBSD/
   ```
1. To download all required packages, including dependencies, for running the receiver program, edit the `/etc/openbsd-mirror.conf`:
   ```
   [OpenBSD]
   version = 7.5

   [aarch64]
   py3-serial = *
   bash = *
   zsh = *
   nano = *
   xz = *
   bzip2 = *
   ```
   You can add additional packages for other architectures as well, for example `amd64`. The file will be updated with the downloaded versions. This will allow the script to download updates if they become available in the mirrors.
1. To start the download, execute the `download_packages` program:
   ```sh
   download_packages --config /etc/openbsd-mirror.conf --directory /home/download/packages
   ```
1. Move or copy the downloaded packages to the diode directory:
   ```sh
   cp -rp /home/download/packages/pub /home/diode/send/www
   ```
1. Wait until all files are transferred. Re-transmit failed chunks/files if required.
1. Optional: If you split files, merge files on the receiver:
   ```sh
   merge_files /home/diode/receive/www/pub/OpenBSD/7.5
   ```

## Upgrade Receiver

To use this procedure, you first need to set up the receiver Pi as an HTTP mirror, see [INSTALL.md](INSTALL.md).

1. Validate that the Python3 package with all its dependencies are transferred before starting the upgrade. If the old Python3 version breaks after the upgrade, you cannot transfer any addtional files through the diode.
1. Upgrade to the newest OpenBSD release:
   ```sh
   sysupgrade
   ```
1. When the system is available again, upgrade the installed packages:
   ```sh
   pkg_add -u
   ```
1. Check if the sender scripts works as expected:
   ```sh
   rcctl start diode_receive
   rcctl check diode_receive
   ```
