# Troubleshooting

## Network Device Inactive

Problem: The diode network device (`axen0` if ASIX AX88179 are used) is inactive.

Solution: Restart the network configuration with `sh /etc/netstart /etc/hostname.axen0`. If it does not help, reboot.
