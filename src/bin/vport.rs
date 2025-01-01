//! Virtual switchport implementation
//!
//! This uses a TAP interface to send/receive
//! the host's traffic to/from the vswitch
//!
//! Usage: vport <vswitch_ip> <vswitch_port>

use l2vpn::utilities::get_frame_log_msg;
use nix::{
    ioctl_write_ptr,
    libc::{ifreq, IFF_NO_PI, IFF_TAP, IFNAMSIZ},
};
use std::{
    env,
    error::Error,
    ffi::{c_char, c_int},
    fs::File,
    io::{Read, Write},
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
const ETHER_HDR: usize = 14;
const ETHER_FCS: usize = 4;
const ETHER_DATA_MIN: usize = ETHER_MIN - ETHER_HDR - ETHER_FCS;

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
ioctl_write_ptr!(tunsetiff, TUNTAP_DRIVER, TUNTAP_SET_FLAGS, c_int);

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

    let mut vport_clone = match clone_vport(&vport) {
        Ok(vport_clone) => vport_clone,
        Err(e) => {
            eprintln!("Failed to clone vport with error: '{}'", e);
            return ExitCode::FAILURE;
        }
    };

    println!("Starting vport");

    /*
     * Start thread which takes packets from
     * tap interface and forwards to vswitch
     */
    let tap_to_vswitch_handle = thread::spawn(move || tap_to_vswitch(&mut vport));

    /*
     * Start thread which takes packets received
     * from the vswitch and forwards them to tap intf
     */
    let vswitch_to_tap_handle = thread::spawn(move || vswitch_to_tap(&mut vport_clone));

    let mut exit_code = ExitCode::SUCCESS;

    /* Wait for tap_to_vswitch thread to finish */
    if let Err(e) = tap_to_vswitch_handle.join() {
        eprintln!("tap_to_vswitch failed with error: '{:?}'", e);
        exit_code = ExitCode::FAILURE;
    }

    /* Wait for vswitch_to_tap thread to finish */
    if let Err(e) = vswitch_to_tap_handle.join() {
        eprintln!("vswitch_to_tap failed with error: '{:?}'", e);
        return ExitCode::FAILURE;
    }

    println!("Terminating vport");

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
        match tunsetiff(tap_file.as_raw_fd(), &mut ifr as *mut _ as *const c_int) {
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
        "Initialised vport using tap interface tap0, and socket {:?}",
        vport.sock
    );

    Ok(vport)
}

/// The tap file in the vport will be read from by one
/// thread and written to by another during the operation
/// of the L2VPN session.
///
/// These operations require a mutable reference to the
/// tap_file File struct, and since we cannot have 2 mutable
/// references to the same File, I wrote this clone_vport
/// method which calls File::try_clone which returns another
/// File struct which refers to the same file handle, thereby
/// creating 2 separate mutable references to the same file handle
/// (but different File structs)
///
/// While reading to and writing from the same file in 2 different
/// threads is normally unsafe, since this file represents /dev/net/tun
/// which is the network I/O interface to a tap/tun interface, this is fine
fn clone_vport(vport: &Vport) -> Result<Vport, Box<dyn Error>> {
    Ok(Vport {
        tap_file: vport.tap_file.try_clone()?,
        vswitch_addr: vport.vswitch_addr,
        /*
         * Reads and writes to UdpSockets only require an
         * immutable reference so this is technically not required,
         * however doing this allows me to bundle everything into
         * the Vport struct which is easier
         */
        sock: vport.sock.try_clone()?,
    })
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
        let mut bytes_read = vport.tap_file.read(&mut buf).unwrap();

        /* If EOF reached, panic */
        if bytes_read == 0 {
            panic!("Reached EOF for /dev/net/tun which should not happen, quitting");
        }

        /* If data is less than 46 bytes, add some padding to the buffer */
        if bytes_read < ETHER_DATA_MIN {
            buf[bytes_read..ETHER_DATA_MIN].fill(0);
            bytes_read = ETHER_DATA_MIN;
        }

        /* Forward received frame to vswitch */
        let bytes_sent = vport
            .sock
            .send_to(&buf[..bytes_read], vport.vswitch_addr)
            .unwrap();

        /* If not all the bytes could be forwarded, fail */
        if bytes_sent != bytes_read {
            panic!(
                "Frame was {} bytes but could only send {} bytes. Quitting.",
                bytes_read, bytes_sent
            );
        }

        /* Log frame */
        println!(
            "Sent frame: {}",
            get_frame_log_msg(&buf[..bytes_read], bytes_read)
        );
    }
}

/// Takes frames received from the vswitch in
/// the L2VPN network and sends to the tap interface
/// which will allow it to exit the emulated L2VPN network
fn vswitch_to_tap(vport: &mut Vport) {
    /* Buffer to store frames received from the vswitch */
    let mut buf = [0u8; ETHER_MTU];

    /*
     * Main loop which takes packets received from the
     * vswitch and forwards them to the tap interface
     */
    loop {
        /* Get virtual ethernet frame from socket */
        let (bytes_read, _) = vport.sock.recv_from(&mut buf).unwrap();

        /* Log any runt frames received, but do not terminate loop */
        if bytes_read < ETHER_MIN {
            eprintln!("Received runt frame which was {} bytes long", bytes_read);
            continue;
        }

        /* Forward virtual ethernet frame to tap interface */
        let bytes_sent = vport.tap_file.write(&buf[..bytes_read]).unwrap();

        /* If not all the bytes could be forwarded, fail */
        if bytes_sent != bytes_read {
            panic!(
                "Received frame with {} bytes but forwarded it with {} bytes. Quitting.",
                bytes_read, bytes_sent
            );
        }

        /* Log frame */
        println!(
            "Received frame: {}",
            get_frame_log_msg(&buf[..bytes_read], bytes_read)
        );
    }
}
