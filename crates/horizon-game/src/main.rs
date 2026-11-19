//! Horizon: native Rust client for Minecraft Java Edition 26.3 gameplay.
#![forbid(unsafe_code)]

/// Identifies this executable as the game for launcher imports. `main` reads it so the
/// linker keeps it; it carries neither an API version nor a per-build id.
static GAME_IDENTITY: [u8; horizon_mod_package::game_identity::MARK.len()] =
    horizon_mod_package::game_identity::MARK;

#[cfg(not(target_os = "horizon"))]
fn main() {
    horizon_guest_stack::prepare_main();
    std::hint::black_box(&GAME_IDENTITY);
    horizon_game::main();
}

#[cfg(target_os = "horizon")]
fn main() {
    std::hint::black_box(&GAME_IDENTITY);
    // A homebrew launch inherits the loader's stack. The game keeps startup
    // and renderer state on its stack across the event loop, so reserve a
    // game thread's stack explicitly. Keep SDK shutdown on the first thread.
    let game = std::thread::Builder::new()
        .name("Render thread".into())
        .stack_size(horizon_guest_stack::HOST_HEADROOM)
        .spawn(horizon_game::main)
        .expect("create the Switch game thread");
    game.join().expect("the Switch game thread panicked");
    horizon_nx::exit_successfully();
}
