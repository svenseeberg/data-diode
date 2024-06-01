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
   rcctl check diode_send
   ```
