//! Virtual switch implementation
//!
//! This executable opens a UDP socket on the host,
//! and handles the Ethernet frames sent to this
//! socket as an Ethernet switch would
//!
//! Usage: vswitch <port>

use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Expected 2 command line arguments and got {}", args.len());
        eprintln!("Usage: vswitch <port>");
        return ExitCode::FAILURE;
    }

    /* Get port number from command line argument */
    let _port = match args[1].parse::<u32>() {
        Ok(port) => port,
        Err(e) => {
            eprintln!("Got error: {}", e);
            eprintln!("Could not parse {} as port number", args[1]);
            return ExitCode::FAILURE;
        }
    };

    ExitCode::SUCCESS
}
