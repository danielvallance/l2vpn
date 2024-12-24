//! Virtual switch implementation
//!
//! This executable opens a UDP socket on the host,
//! and handles the Ethernet frames sent to this
//! socket as an Ethernet switch would
//!
//! Usage: vswitch <port>

use std::{
    collections::HashMap,
    env,
    net::{SocketAddr, UdpSocket},
    process::ExitCode,
};

const MTU: usize = 1518;

/// Returns string representation of passed MAC bytes
fn mac_string(mac: &[u8]) -> String {
    mac.iter()
        .take(6)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(":")
}

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
            eprintln!("Got error: {}", e);
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
        let (no_of_bytes, src_vport) = match socket.recv_from(&mut buf) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Got error: {}", e);
                eprintln!("Exiting.");
                return ExitCode::FAILURE;
            }
        };

        /* Extract ethernet frame from entire buffer */
        let eth_frame = &buf[..no_of_bytes];

        /* Extract src and dst MAC addresses */
        let dst_mac = &eth_frame[..6];
        let src_mac = &eth_frame[6..12];

        println!(
            "vswitch: src_vport={}, src_mac={}, dst_mac={}",
            src_vport,
            mac_string(src_mac),
            mac_string(dst_mac)
        );

        /*
         * If entry in MAC table contradicts source of
         * received frame, then update table
         */
        if mac_table.get(src_mac) != Some(&src_vport) {
            mac_table.insert(src_mac.try_into().unwrap(), src_vport);

            /* Print updated MAC table */
            println!("MAC table:\n{:?}", mac_table);
        }
    }
}
