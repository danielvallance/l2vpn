# l2vpn

Basic L2VPN implementation in Rust.

This repository contains the code for virtual ports which make use of tap interfaces to insert/extract packets to/from the emulated L2VPN network. Additionally it contains the code for a virtual ethernet switch within the emulated L2VPN network, which communicates with the virtual ports over UDP, and handles frame forwarding and ARP mappings.

A Docker compose file is included so that this can be run on any platform.

It is based off the implementation here: https://github.com/peiyuanix/build-your-own-zerotier

# Building and Running

## On linux host(s)

```cargo build``` will build both the vswitch and vport executables.

```cargo run --bin vswitch <port>``` will run the vswitch executable, and expose it on the given port.

Running the vport requires that a tap interface tap0 is configured. This can be done by running ```./setup.sh <tap_intf_ip>```, which will give tap0 the passed ip address.

After this, the vport executable can be run with ```cargo run --bin vport <vswitch_ip> <vswitch_port>```, and it will communicate with the vswitch accessible at the given IP/port.

## Docker Compose

Since the vport code uses tun/tap mechanisms which are Linux-specific, I created a Docker compose file to allow this code to be run on other platforms.

Running ```docker compose up``` will spin up 3 alpine Linux containers on the same docker bridge network. The vport_a and vport_b containers will have run ./setup.sh, so there is no need to repeat this.

Once the containers have come up, a shell can be opened with ```docker compose exec <container_name> sh```, and then the ```cargo run --bin...``` commands can be run as above.

Once the vswitch and vports have been set up, you can open another shell into the vports, and reach each other at their tap0 IP addresses over the emulated L2VPN network. (These will be the addresses passed to ./setup.sh. In the docker compose file, these are 10.0.0.1/24 and 10.0.0.2/24 for vport_a and vport_b respectively.)

# Demo

Here is a basic demo which:
* Uses the Docker Compose to spin up the alpine containers
* Opens a shell on the vswitch container
* Builds and runs the vswitch executable, and exposes it on port 3000
* Opens 2 shells on each of the vport containers
* Builds and runs the vport executables, pointing them to the IP/port of the vswitch
* Shows that the vports can ping each other using the tap0 addresses
* Shows the logs on the vswitch and vport, showing that the traffic traversed the emulated L2VPN network
* Stops the vswitch, and retries the pings again, proving that the vswitch is necessary for the pings to work over the L2VPN network
* Tears down the emulated L2VPN network


https://github.com/user-attachments/assets/e882b66e-e32e-4a2d-a063-92b779681b0d

