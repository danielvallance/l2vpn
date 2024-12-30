//! Virtual switchport implementation
//!
//! This uses a TAP interface to send/receive
//! the host's traffic to/from the vswitch
//!
//! Usage: vport <vswitch_ip> <vswitch_port>

use l2vpn::utilities::mac_string;
use nix::{
    ioctl_write_ptr,
    libc::{ifreq, IFF_NO_PI, IFF_TAP, IFNAMSIZ},
};
use std::{
    env,
    error::Error,
    ffi::c_char,
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    os::fd::AsRawFd,
    process::ExitCode,
    thread,
};

/*
 * These constants are defined in linux/if_tun.h
 * and ioctl uses them to identify that an operation
 * should affect the tuntap driver, and that it should
 * be setting interface flags respectively
 */
const TUNTAP_DRIVER: u8 = b'T';
const TUNTAP_SET_FLAGS: u8 = 202;

const ETHER_MTU: usize = 1518;
const ETHER_MIN: usize = 64;

/*
 * Struct which contains information required for vport
 * to communicate with vswitch
 */
struct Vport {
    tap_file: File,
    vswitch_addr: SocketAddr,
    sock: UdpSocket,
}

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
    let vswitch_ip = match args[1].parse::<Ipv4Addr>() {
        Ok(ip) => ip,
        Err(e) => {
            eprintln!(
                "Got error while parsing IPv4 address from command line argument: '{}'",
                e
            );
            eprintln!("Could not parse '{}' as IPv4 address", args[1]);
            return ExitCode::FAILURE;
        }
    };

    /* Get port number from command line argument */
    let vswitch_port = match args[2].parse::<u16>() {
        Ok(vswitch_port) => vswitch_port,
        Err(e) => {
            eprintln!(
                "Got error while parsing port command line argument: '{}'",
                e
            );
            eprintln!("Could not parse '{}' as port number", args[2]);
            return ExitCode::FAILURE;
        }
    };

    /* Initialise vport struct */
    let mut vport = match initialise_vport(vswitch_ip, vswitch_port) {
        Ok(vport) => vport,
        Err(e) => {
            eprintln!("Got error while initialising vport: '{}'", e);
            eprintln!("Quitting");
            return ExitCode::FAILURE;
        }
    };

    /*
     * Start thread which takes packets from
     * tap interface and forwards to vswitch
     */
    let tap_to_vswitch_handle = thread::spawn(move || tap_to_vswitch(&mut vport));

    let mut exit_code = ExitCode::SUCCESS;

    /* Wait for tap_to_vswitch thread to finish */
    if let Err(e) = tap_to_vswitch_handle.join() {
        eprintln!("tap_to_vswitch failed with error: '{:?}'", e);
        exit_code = ExitCode::FAILURE;
    }

    exit_code
}

/// Create and configure tap interface which will
/// take the traffic that the underlay interface handles
/// and insert it into the L2VPN network we are setting up
///
/// Returns the /dev/net/tun file handler on success
fn create_tap_intf(ul_intf: &str) -> Result<File, Box<dyn Error>> {
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

/// Initialise vport struct so that it is
/// ready to communicate on the L2VPN network
fn initialise_vport(vswitch_ip: Ipv4Addr, vswitch_port: u16) -> Result<Vport, Box<dyn Error>> {
    /* Configure tap interface tap0 and return file handle to it */
    let tap_file = create_tap_intf("tap0")?;

    /*
     * Create UDP socket which the vport will use to communicate with the vswitch
     *
     * It communicates on any available IP and a random ephemeral port, which is fine
     * as the other vport requires the address of the tap interface, not this socket
     */
    let sock = UdpSocket::bind("0.0.0.0:0".to_string())?;

    /*
     * Store address of vswitch as for the L2VPN to function
     * properly, it must be able to communicate with the vswitch
     */
    let vswitch_addr = SocketAddr::new(IpAddr::V4(vswitch_ip), vswitch_port);

    let vport = Vport {
        tap_file,
        sock,
        vswitch_addr,
    };

    println!(
        "Initialised vport using tap interface tun0, and socket {:?}",
        vport.sock
    );

    Ok(vport)
}

/// Take frame which the tap interface receives
/// and inject it into the L2VPN network by forwarding
/// it to the vswitch
fn tap_to_vswitch(vport: &mut Vport) {
    /* Buffer to store frames the tap interface receives */
    let mut buf = [0u8; ETHER_MTU];

    /*
     * Main loop which takes packets which the tap
     * interface receives and forwards them to the vswitch
     */
    loop {
        /* Fill buffer with bytes read from tap interface */
        let bytes_read = vport.tap_file.read(&mut buf).unwrap();

        /* If EOF reached, panic */
        if bytes_read == 0 {
            panic!("Reached EOF for /dev/net/tun which should not happen, quitting");
        }

        /* Log any runt frames received, but do not terminate loop */
        if bytes_read < ETHER_MIN {
            eprintln!("Received runt frame which was {} bytes long", bytes_read);
            continue;
        }

        /* Forward received frame to vswitch */
        let bytes_sent = vport
            .sock
            .send_to(&buf[..bytes_read], vport.vswitch_addr)
            .unwrap();

        /* If not all the bytes could be forwarded, fail */
        if bytes_sent != bytes_read {
            panic!(
                "Received frame with {} bytes but forwarded it with {} bytes. Quitting.",
                bytes_read, bytes_sent
            );
        }

        /* Log frame */
        let dst_mac = mac_string(&buf[0..6]);
        let src_mac = mac_string(&buf[6..12]);
        let ether_type = ((buf[12] as u16) << 8) + buf[13] as u16;
        println!(
            "Frame forwarded to vswitch: dst={}, src={}, type={}, size={}",
            dst_mac, src_mac, ether_type, bytes_read
        );
    }
}
