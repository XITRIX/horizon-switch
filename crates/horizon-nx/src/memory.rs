//! The memory the game may use: about 3.2 GB when started over a game (holding R), about
//! 400 MB from the album.

unsafe extern "C" {
    fn nxshim_memory(total: *mut u64, used: *mut u64);
}

/// (total, used) bytes for this process.
pub fn usage() -> (u64, u64) {
    let (mut total, mut used) = (0, 0);
    // SAFETY: two plain svcGetInfo queries into valid pointers.
    unsafe { nxshim_memory(&mut total, &mut used) };
    (total, used)
}

unsafe extern "C" {
    fn nxshim_sd_space(total: *mut i64, free: *mut i64) -> bool;
}

/// Total and free bytes on the SD card.
pub fn sd_space() -> Option<(u64, u64)> {
    let (mut total, mut free) = (0, 0);
    // SAFETY: both output pointers are writable for the duration of the call.
    (unsafe { nxshim_sd_space(&mut total, &mut free) })
        .then(|| Some((total.try_into().ok()?, free.try_into().ok()?)))?
}
