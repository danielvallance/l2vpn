//! Virtual switch implementation
//!
//! This executable opens a UDP socket on the host,
//! and handles the Ethernet frames sent to this
//! socket as an Ethernet switch would
//!
//! Usage: vswitch <port>

use std::{env, net::UdpSocket, process::ExitCode};

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

    loop {
        let (_no_of_bytes, _src) = match socket.recv_from(&mut buf) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Got error: {}", e);
                eprintln!("Exiting.");
                return ExitCode::FAILURE;
            }
        };
    }
}
