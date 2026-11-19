//! The Switch profile picker, shown before the game takes the default window.
#[allow(dead_code)] // The desktop launcher also uses this module for account refresh.
#[path = "microsoft_account.rs"]
mod account;

use horizon_client::auth::{self, AuthSession};
use horizon_nx::{
    console::Console,
    pad::{Hid, Pad, button},
};
use qrcode::{Color, QrCode};
use std::{
    fmt::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const OFFLINE: &str = "A  Play Offline";
const SIGN_IN: &str = "X  Sign in with Microsoft";

enum Update {
    Code { url: String, code: String },
    Finished(Result<AuthSession, String>),
}

struct SignIn {
    updates: mpsc::Receiver<Update>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for SignIn {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl SignIn {
    fn start(client_id: String) -> std::io::Result<Self> {
        let (sender, updates) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        thread::Builder::new()
            .name("Microsoft sign-in".into())
            .spawn(move || {
                let cancelled = || worker_cancelled.load(Ordering::Acquire);
                let sign_in = || {
                    let endpoints = account::Endpoints::live();
                    let code = account::device_code(&endpoints, &client_id)?;
                    let url = code.verification_uri.trim_end_matches('/');
                    if !matches!(
                        url,
                        "https://www.microsoft.com/link"
                            | "https://microsoft.com/link"
                            | "https://www.microsoft.com/devicelogin"
                            | "https://microsoft.com/devicelogin"
                    ) || !(1..=16).contains(&code.user_code.len())
                        || !code
                            .user_code
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        return Err(
                            "Microsoft supplied an invalid verification link or code.".into()
                        );
                    }
                    sender
                        .send(Update::Code {
                            url: url.into(),
                            code: code.user_code.clone(),
                        })
                        .map_err(|_| "Sign-in cancelled.")?;
                    let tokens =
                        account::wait_for_sign_in(&endpoints, &client_id, &code, &cancelled)?;
                    if cancelled() {
                        return Err("Sign-in cancelled.".into());
                    }
                    let (profile, secrets) = account::finish_sign_in(&endpoints, tokens)?;
                    let uuid = auth::parse_uuid(&profile.uuid)
                        .ok_or("Minecraft returned an invalid profile ID.")?;
                    if profile.name.is_empty()
                        || profile.name.len() > 16
                        || !profile
                            .name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    {
                        return Err("Minecraft returned an invalid player name.".into());
                    }
                    let mut session =
                        AuthSession::new(&profile.name, uuid, &secrets.minecraft_token);
                    session.xuid = secrets.xuid;
                    Ok(session)
                };
                let result = sign_in();
                if !cancelled() {
                    let _ = sender.send(Update::Finished(result));
                }
            })?;
        Ok(Self { updates, cancelled })
    }
}

fn screen_at(body: &str, controls: &str, row: usize) -> String {
    format!("\x1b[0m\x1b[2J\x1b[2;5HHorizon\x1b[4;5H{body}\x1b[{row};5H{controls}")
}

fn screen(body: &str, controls: &str) -> String {
    screen_at(body, controls, 9)
}

fn show_code(url: &str, code: &str) -> String {
    let mut body = format!("Scan with your phone, then enter {code}\x1b[5;5H{url}");
    if let Ok(qr) = QrCode::new(url) {
        let width = qr.width();
        let colors = qr.to_colors();
        // The console uses square cells. Leave the QR code's quiet zone white.
        for y in 0..width + 8 {
            let _ = write!(body, "\x1b[{};5H", y + 6);
            let mut previous = None;
            for x in 0..width + 8 {
                let dark = (4..width + 4).contains(&x)
                    && (4..width + 4).contains(&y)
                    && colors[(y - 4) * width + (x - 4)] == Color::Dark;
                if previous != Some(dark) {
                    body.push_str(if dark { "\x1b[40m" } else { "\x1b[47m" });
                    previous = Some(dark);
                }
                body.push(' ');
            }
            body.push_str("\x1b[0m");
        }
    }
    screen_at(&body, "A  Play Offline        B  Cancel Sign-in", 43)
}

/// Returns false if the player quits before entering the game.
pub(super) fn choose_profile() -> bool {
    let _hid = match Hid::open() {
        Ok(hid) => hid,
        Err(error) => {
            log::warn!("Horizon: could not open controller input for sign-in (0x{error:08x})");
            return true;
        }
    };
    let client_id = account::client_id();
    let mut console = Console::open();
    let mut pad = Pad::new();
    let mut previous_buttons = pad.update().buttons;
    let start = screen(
        "Choose how to play",
        &format!("{OFFLINE}        {SIGN_IN}        B  Quit"),
    );
    let mut view = start.clone();
    let mut sign_in: Option<SignIn> = None;
    while console.show(&view) {
        view.clear();
        let buttons = pad.update().buttons;
        let pressed = buttons & !previous_buttons;
        previous_buttons = buttons;
        if pressed & button::A != 0 {
            return true;
        }
        if pressed & button::B != 0 {
            if sign_in.take().is_some() {
                view.clone_from(&start);
            } else {
                return false;
            }
        }
        if pressed & button::X != 0 && sign_in.is_none() {
            match client_id.clone().map(SignIn::start) {
                Some(Ok(request)) => {
                    sign_in = Some(request);
                    view = screen(
                        "Requesting a sign-in code...",
                        "A  Play Offline        B  Cancel Sign-in",
                    );
                }
                Some(Err(_)) => {
                    view = screen(
                        "Could not start sign-in. Try again.",
                        &format!("{OFFLINE}        {SIGN_IN}"),
                    )
                }
                None => view = screen("Microsoft sign-in needs a Mojang-approved app ID.", OFFLINE),
            }
        }
        if let Some(request) = &sign_in {
            match request.updates.try_recv() {
                Ok(Update::Code { url, code }) => view = show_code(&url, &code),
                Ok(Update::Finished(Ok(session))) => {
                    auth::set_current(session);
                    return true;
                }
                Ok(Update::Finished(Err(error))) => {
                    view = screen(&error, &format!("{OFFLINE}        X  Try Again"));
                    sign_in = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    view = screen(
                        "Sign-in ended. Try again.",
                        &format!("{OFFLINE}        X  Try Again"),
                    );
                    sign_in = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        thread::sleep(Duration::from_millis(16));
    }
    false
}
