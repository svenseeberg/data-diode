# Troubleshooting

## `download_packages` Fails to Resolve Dependencies on New Major Release

Problem: After switching to a new major release, for example 7.6 to 7.7, the `download_packages` tool fails to resolve some dependencies.

Solution: Remove the full `[amd64/dependencies]` section from the config file.

## Network Device Inactive

Problem: The diode network device (`axen0` if ASIX AX88179 are used) is inactive.

Solution: Restart the network configuration with `sh /etc/netstart /etc/hostname.axen0`. If it does not help, reboot.
