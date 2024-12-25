//! Virtual switchport implementation
//!
//! This uses a TAP interface to send/receive
//! the host's traffic to/from the vswitch
//!
//! Usage: vport <vswitch_ip> <vswitch_port>

use std::{env, net::IpAddr, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Expected 3 command line arguments and got {}", args.len());
        eprintln!("Usage: vport <vswitch_ip> <vswitch_port>");
        return ExitCode::FAILURE;
    }

    /* Get vswitch IP from command line argument */
    let _vswitch_ip = match args[1].parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(e) => {
            eprintln!(
                "Got error while parsing IP address from command line argument: {}",
                e
            );
            eprintln!("Could not parse {} as IP address", args[1]);
            return ExitCode::FAILURE;
        }
    };

    /* Get port number from command line argument */
    let _port = match args[2].parse::<u32>() {
        Ok(port) => port,
        Err(e) => {
            eprintln!("Got error while parsing port command line argument: {}", e);
            eprintln!("Could not parse {} as port number", args[2]);
            return ExitCode::FAILURE;
        }
    };

    ExitCode::SUCCESS
}
