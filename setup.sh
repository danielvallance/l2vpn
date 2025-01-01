#!/usr/bin/env sh

# This script contains setup required for the vports
# to participate in the L2VPN network

[ -z "$1" ] && echo "Usage: ./setup.sh <ipv4_addr>" && exit 1

# contains 'ip tuntap' command
apk add iproute2

# Create tuntap device
mkdir -p /dev/net
mknod /dev/net/tun c 10 200
chmod 600 /dev/net/tun

# Create tap interface tap0
ip tuntap add dev tap0 mode tap
ip link set tap0 up
ip addr add $1 dev tap0

echo "Finished setup.sh"