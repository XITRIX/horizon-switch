//! The libnx services Horizon uses on the Nintendo Switch, behind safe wrappers: the game's
//! crates keep `#![forbid(unsafe_code)]`. Builds on every target; off the Switch it is empty.
#![cfg(target_os = "horizon")]

pub mod alloc;
pub mod audio;
pub mod console;
pub mod memory;
pub mod pad;
pub mod romfs;

unsafe extern "C" {
    fn nxshim_exit_successfully() -> !;
    fn nxshim_net_init() -> u32;
    fn nxshim_local_ipv4(address: *mut u32) -> u32;
}

/// Starts BSD sockets and DNS, returning the SDK result code on failure.
pub fn net_init() -> Result<(), u32> {
    // SAFETY: libnx's socket setup, once at startup.
    match unsafe { nxshim_net_init() } {
        0 => Ok(()),
        result => Err(result),
    }
}

/// The active interface's IPv4 address, as returned by the SDK's network service.
pub fn local_ipv4() -> Option<std::net::Ipv4Addr> {
    let mut address = 0;
    // SAFETY: the SDK writes one network-order IPv4 address to this valid pointer.
    let result = unsafe { nxshim_local_ipv4(&mut address) };
    (result == 0 && address != 0).then(|| std::net::Ipv4Addr::from(address.to_ne_bytes()))
}

/// Finishes a successful run after the game has saved and released its resources.
pub fn exit_successfully() -> ! {
    // SAFETY: called on the main thread after the game loop and its resources are gone.
    unsafe { nxshim_exit_successfully() }
}
