//! Horizon: native Rust client for Minecraft Java Edition 26.3 gameplay.
#![forbid(unsafe_code)]

/// Identifies this executable as the game for launcher imports. `main` reads it so the
/// linker keeps it; it carries neither an API version nor a per-build id.
static GAME_IDENTITY: [u8; horizon_mod_package::game_identity::MARK.len()] =
    horizon_mod_package::game_identity::MARK;

fn main() {
    #[cfg(not(target_os = "horizon"))]
    horizon_guest_stack::prepare_main();
    std::hint::black_box(&GAME_IDENTITY);
    horizon_game::main();
    #[cfg(target_os = "horizon")]
    horizon_nx::exit_successfully();
}
