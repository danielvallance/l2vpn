//! Virtual switchport implementation
//!
//! This uses a TAP interface to send/receive
//! the host's traffic to/from the vswitch
//!
//! Usage: vport <vswitch_ip> <vswitch_port>

use nix::{
    ioctl_write_ptr,
    libc::{ifreq, IFF_NO_PI, IFF_TAP, IFNAMSIZ},
};
use std::{
    env, error::Error, ffi::c_char, fs::File, net::IpAddr, os::fd::AsRawFd, process::ExitCode,
};

/*
 * These constants are defined in linux/if_tun.h
 * and ioctl uses them to identify that an operation
 * should affect the tuntap driver, and that it should
 * be setting interface flags respectively
 */
const TUNTAP_DRIVER: u8 = b'T';
const TUNTAP_SET_FLAGS: u8 = 202;

/*
 * This macro generates a function called tunsetiff
 * which is a wrapper around the ioctl call which points
 * /dev/net/tun to the device specified in the ifreq struct,
 * and configures it with the flags set in the ifreq struct
 */
ioctl_write_ptr!(tunsetiff, TUNTAP_DRIVER, TUNTAP_SET_FLAGS, ifreq);

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
                "Got error while parsing IP address from command line argument: '{}'",
                e
            );
            eprintln!("Could not parse '{}' as IP address", args[1]);
            return ExitCode::FAILURE;
        }
    };

    /* Get port number from command line argument */
    let _port = match args[2].parse::<u32>() {
        Ok(port) => port,
        Err(e) => {
            eprintln!("Got error while parsing port command line argument: '{}'", e);
            eprintln!("Could not parse '{}' as port number", args[2]);
            return ExitCode::FAILURE;
        }
    };

    ExitCode::SUCCESS
}

/// Create and configure tap interface which will
/// take the traffic that the underlay interface handles
/// and insert it into the L2VPN network we are setting up
///
/// Returns the /dev/net/tun file handler on success
fn _create_tap_intf(ul_intf: &str) -> Result<File, Box<dyn Error>> {

    /*
     * Ensure the ul_intf name is valid ASCII and is <= IFNAMSIZ bytes
     *
     * The Linux/C network stack expects ASCII interface names, and if
     * the interface name is ASCII encoded, then every UTF-8 char is 1 byte,
     * so we do not need to worry about the distinction after this.
     */
    if !ul_intf.is_ascii() {
        return Err(format!("Interface name is not valid ASCII: '{}'", ul_intf).into());
    }

    if ul_intf.len() > IFNAMSIZ {
        return Err(format!("Interface name longer than IFNAMSIZ(16): '{}'", ul_intf).into());
    }

    /* Open the /dev/net/tun file which is the interface to the tun/tap driver */
    let tap_file = File::options()
        .read(true)
        .write(true)
        .open("/dev/net/tun")?;

    /*
     * Initialise the ifreq struct which indicates the
     * underlay interface we are going to use, and specifies
     * the IFF_TAP and IFF_NO_PI flags which indicate we want
     * to configure it as an L2 tap interface, and that we want
     * it to handle raw data without any extra headers
     */
    let mut ifr: ifreq = unsafe { std::mem::zeroed() };
    ifr.ifr_ifru.ifru_flags = (IFF_TAP | IFF_NO_PI) as i16;
    for (i, b) in ul_intf.bytes().enumerate() {
        ifr.ifr_name[i] = b as c_char;
    }

    /* Perform the ioctl call to configure the tap interface */
    unsafe {
        match tunsetiff(tap_file.as_raw_fd(), &ifr) {
            Ok(_) => Ok(tap_file),
            Err(e) => Err(format!("tunsetiff failed with error: '{}'", e).into()),
        }
    }
}
