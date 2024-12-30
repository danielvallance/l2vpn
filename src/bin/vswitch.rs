//! Virtual switch implementation
//!
//! This executable opens a UDP socket on the host,
//! and handles the Ethernet frames sent to this
//! socket as an Ethernet switch would
//!
//! Usage: vswitch <port>

use l2vpn::utilities::mac_string;
use std::{
    collections::HashMap,
    env,
    net::{SocketAddr, UdpSocket},
    process::ExitCode,
};

const MTU: usize = 1518;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Expected 2 command line arguments and got {}", args.len());
        eprintln!("Usage: vswitch <port>");
        return ExitCode::FAILURE;
    }

    /* Get port number from command line argument */
    let port = match args[1].parse::<u32>() {
        Ok(port) => port,
        Err(e) => {
            eprintln!("Got error while parsing port command line argument: {}", e);
            eprintln!("Could not parse {} as port number", args[1]);
            return ExitCode::FAILURE;
        }
    };

    /* Create UDP socket to receive Ethernet frames on */
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("Got error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    /* Buffer to store received frames */
    let mut buf: [u8; MTU] = [0; MTU];

    /*
     * I should implement some sort of ageing mechanism
     * to reclaim unused memory however since this is
     * a small project I will skip over this
     */
    let mut mac_table: HashMap<[u8; 6], SocketAddr> = HashMap::new();

    loop {
        /* Get virtual ethernet frame from socket */
        let (no_of_bytes, src_vport) = match socket.recv_from(&mut buf) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Got error while listening on socket: {}", e);
                eprintln!("Quitting");
                return ExitCode::FAILURE;
            }
        };

        /* Extract ethernet frame from entire buffer */
        let eth_frame = &buf[..no_of_bytes];

        /* Extract src and dst MAC addresses */
        let dst_mac: [u8; 6] = eth_frame[..6].try_into().unwrap();
        let src_mac: [u8; 6] = eth_frame[6..12].try_into().unwrap();

        println!(
            "vswitch: src_vport={}, src_mac={}, dst_mac={}",
            src_vport,
            mac_string(&src_mac),
            mac_string(&dst_mac)
        );

        /*
         * If entry in MAC table contradicts source of
         * received frame, then update table
         */
        if mac_table.get(&src_mac) != Some(&src_vport) {
            mac_table.insert(src_mac, src_vport);

            /* Print updated MAC table */
            println!("MAC table:\n{:?}", &mac_table);
        }

        /*
         * Forward the received packet out the appropriate vport(s)
         */
        match mac_table.get(&dst_mac) {
            /* If the vport for the dst_mac is known, forward it */
            Some(dst_vport) => {
                if let Err(e) = socket.send_to(&buf, dst_vport) {
                    eprintln!("Got error while forwarding frame unicast: {}", e);
                    eprintln!("Quitting");
                    return ExitCode::FAILURE;
                }
                println!("Unicast forwarded to: {}", mac_string(&dst_mac));
            }
            None => {
                /*
                 * If the dst_mac is the broadcast MAC, send to
                 * every known vport except the src_vport
                 */
                if dst_mac == [0xFFu8; 6] {
                    for (_, dst_vport) in mac_table.iter().filter(|(mac, _)| **mac != src_mac) {
                        if let Err(e) = socket.send_to(&buf, dst_vport) {
                            eprintln!("Got error while forwarding frame broadcast: {}", e);
                            eprintln!("Quitting");
                            return ExitCode::FAILURE;
                        }
                        println!("Broadcast forwarded to: {}", mac_string(&dst_mac));
                    }
                } else {
                    /*
                     * Discard frame if unicast destination MAC is unrecognised, as
                     * ARP resolution is outside the scope of this project
                     */
                    println!("Dropped frame");
                }
            }
        }
    }
}
