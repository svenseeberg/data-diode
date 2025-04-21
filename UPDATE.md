# Updating OpenBSD

Updating OpenBSD can break the diode if the installed Python3 packages are not updated along with the main release.

## Upgrade Sender

To update OpenBSD on the diode, first update the sender:

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

1. Download the tgz files for updating OpenBSD:
   ```sh
   BSD_VERSION=7.5
   wget -nH -nc -r --no-parent https://cdn.openbsd.org/pub/OpenBSD/$BSD_VERSION/arm64/ -R "index.html*" --reject iso,img
   ```
1. Split files larger than 10MB into chunks:
   ```sh
   split_files pub/OpenBSD/$BSD_VERSION
   ```
1. Copy files to diode directory:
   ```sh
   cp -r pub/OpenBSD/$BSD_VERSION /var/www/diode/pub/OpenBSD/
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
   cp -rp /home/download/packages/pub /var/www/diode/
   ```
1. Wait until all files are transferred. Re-transmit failed chunks/files if required.
1. Merge files on the receiver:
   ```sh
   cd /var/www/diode
   merge_files /var/www/diode/pub/OpenBSD/7.5
   ```

## Set Up OpenBSD Mirror on Receiver

To serve received files in the internal network, configure httpd:

```
server "default" {
   listen on * port 80
   directory auto index
   root "/www"
}
```

## Upgrade Receiver
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
   rcctl start diode_receive
   rcctl check diode_receive
   ```
