//! The game as the platform runs it, for an executable to start (docs/layers.md, "How a Horizon
//! crate plugs in"). The official executable (`horizon-official`, Horizon layer) registers its
//! hooks (`horizon_platform::hooks`) and then calls [`main`]; it never names an engine crate.
//!
//! An executable built on it:
//!
//! ```ignore
//! static GAME_IDENTITY: [u8; horizon_game::GAME_IDENTITY.len()] = horizon_game::GAME_IDENTITY;
//!
//! fn main() {
//!     horizon_game::prepare_main_stack();
//!     std::hint::black_box(&GAME_IDENTITY);
//!     my_crate::register();
//!     horizon_game::main();
//! }
//! ```
//!
//! The platform's own parts register here, in [`main`], as they leave the engine (layer plan M6).
#![forbid(unsafe_code)]

/// Identifies an executable as the game for launcher imports, without an API version or build id.
pub const GAME_IDENTITY: [u8; horizon_mod_package::game_identity::MARK.len()] =
    horizon_mod_package::game_identity::MARK;

/// Runs the game with the hooks registered so far, and returns when it ends. Register hooks
/// (`horizon_platform::hooks::register`) before calling it.
#[cfg(not(target_os = "horizon"))]
pub fn main() {
    horizon_guest_stack::prepare_main();
    horizon_mod_host::exit_if_compile_child();
    let startup = horizon_client::entry::prepare();
    std::hint::black_box(&horizon_mod_package::holder::CAPABILITY);
    horizon_mod_host::main_with(Vec::new(), &[], &[], startup);
}

/// Establish the executable main thread stack before initialization.
pub use horizon_guest_stack::prepare_main as prepare_main_stack;

/// Start the game on Switch, where external code mods and launcher processes are unavailable.
#[cfg(target_os = "horizon")]
pub fn main() {
    use horizon_client::entry::{self, CodeRuntime};

    let startup = entry::prepare();
    horizon_engine_slots::install_platform(horizon_engine_slots::Platform {
        install_packages: mod_api::__private::install_packages,
        install_registration_data: mod_api::__private::install_registration_data,
        install_parts: mod_api::__private::install_parts,
        profiler_is_enabled: || false,
        profiler_start: || false,
        profiler_stop: || {},
        profiler_summary: String::new,
        profiler_firefox_profile: || Err("Profiling is unavailable on Nintendo Switch".into()),
    })
    .expect("the Switch platform is installed once");
    entry::start(
        horizon_mod_package::runtime::Runtime::default(),
        |_, _| (Vec::new(), ()),
        CodeRuntime {
            initialize: |_| {},
            refusal: |_| None,
            startup_refused: |_| false,
        },
        startup,
    );
}
