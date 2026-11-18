//! Microsoft device-code authorization and Minecraft account verification shared with the launcher.
//! Token storage and the sign-in UI are handled by each caller.
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) const SCOPE: &str = "XboxLive.signin offline_access";
pub const NOT_CONFIGURED: &str = "Microsoft sign-in is coming soon. Until then the game plays offline: single player, \
     LAN and servers in offline mode work.";
pub(super) const SIGN_IN_AGAIN: &str = "Your Microsoft sign-in expired. Sign in again.";
/// The Minecraft token is renewed when it has less than this left.
const RENEW_BEFORE: u64 = 3600;

/// The configured Azure app, if any: the run-time variable wins over the build-time one, and an
/// empty value means none.
pub fn client_id() -> Option<String> {
    std::env::var("HORIZON_MSA_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("HORIZON_MSA_CLIENT_ID").map(str::to_owned))
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// Where the requests go: the real services, or a mock in tests.
#[derive(Clone, Debug)]
pub struct Endpoints {
    /// The OAuth 2.0 v2 endpoints for personal accounts (`…/devicecode`, `…/token`).
    pub microsoft: String,
    pub xbox_user: String,
    pub xsts: String,
    /// api.minecraftservices.com.
    pub minecraft: String,
}

impl Endpoints {
    pub fn live() -> Self {
        // Development builds only: `HORIZON_MSA_MICROSOFT_ENDPOINT` points the device-code and
        // token requests at a local mock, to see and screenshot the sign-in panel.
        let microsoft = std::env::var("HORIZON_MSA_MICROSOFT_ENDPOINT")
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .unwrap_or_else(|| "https://login.microsoftonline.com/consumers/oauth2/v2.0".into());
        Self {
            microsoft,
            xbox_user: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            minecraft: "https://api.minecraftservices.com".into(),
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("horizon/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// One request's result: the status and the JSON body (empty when there is none). Transport
/// errors name the service, never the URL's query or the request.
fn send(request: ureq::Request, body: Option<Body>, service: &str) -> Result<(u16, Value), String> {
    let result = match body {
        None => request.call(),
        Some(Body::Form(fields)) => request.send_form(&fields),
        Some(Body::Json(value)) => request.set("Accept", "application/json").send_json(value),
    };
    let response = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(_)) => {
            return Err(format!(
                "Could not reach {service}. Check your internet connection."
            ));
        }
    };
    let status = response.status();
    let text = response.into_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

enum Body<'a> {
    Form(Vec<(&'a str, &'a str)>),
    Json(Value),
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ----- The device-code flow ----------------------------------------------------------------------

/// What the player types at the verification page.
#[derive(Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_at: u64,
    device_code: String,
}

impl std::fmt::Debug for DeviceCode {
    // The device code can be traded for tokens once the player approves: never shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .finish_non_exhaustive()
    }
}

/// The Microsoft tokens; the access token only lasts for the Xbox step.
pub struct MsTokens {
    pub(super) access: String,
    pub(super) refresh: String,
}

impl std::fmt::Debug for MsTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MsTokens(<hidden>)")
    }
}

pub enum Poll {
    Pending,
    /// The server asked to poll less often: wait 5 s more.
    SlowDown,
    Done(MsTokens),
}

pub fn device_code(ep: &Endpoints, client_id: &str) -> Result<DeviceCode, String> {
    let (status, v) = send(
        agent().post(&format!("{}/devicecode", ep.microsoft)),
        Some(Body::Form(vec![("client_id", client_id), ("scope", SCOPE)])),
        "Microsoft",
    )?;
    match (
        status,
        str_field(&v, "user_code"),
        str_field(&v, "device_code"),
    ) {
        (200, Some(user_code), Some(device_code)) => Ok(DeviceCode {
            user_code,
            verification_uri: str_field(&v, "verification_uri")
                .unwrap_or_else(|| "https://www.microsoft.com/link".into()),
            interval: v
                .get("interval")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1),
            expires_at: now() + v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
            device_code,
        }),
        _ => Err(oauth_error(&v)),
    }
}

/// The OAuth error codes as messages for the player.
pub(super) fn oauth_error(v: &Value) -> String {
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_declined") => "Sign-in was declined in the browser.".into(),
        Some("expired_token") => "The sign-in code expired. Start again.".into(),
        Some("bad_verification_code") => "The sign-in code was not recognised. Start again.".into(),
        Some("invalid_grant") => SIGN_IN_AGAIN.into(),
        Some("invalid_client" | "unauthorized_client") => {
            "Microsoft refused this launcher's app registration (client ID).".into()
        }
        _ => "Microsoft sign-in failed.".into(),
    }
}

/// One poll of the token endpoint while the player signs in.
pub fn poll(ep: &Endpoints, client_id: &str, code: &DeviceCode) -> Result<Poll, String> {
    let (status, v) = send(
        agent().post(&format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", &code.device_code),
        ])),
        "Microsoft",
    )?;
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => return Ok(Poll::Pending),
        Some("slow_down") => return Ok(Poll::SlowDown),
        _ => {}
    }
    ms_tokens(status, &v).map(Poll::Done)
}

fn ms_tokens(status: u16, v: &Value) -> Result<MsTokens, String> {
    match (
        status,
        str_field(v, "access_token"),
        str_field(v, "refresh_token"),
    ) {
        (200, Some(access), Some(refresh)) => Ok(MsTokens { access, refresh }),
        _ => Err(oauth_error(v)),
    }
}

/// Wait for the player to finish in the browser, polling as asked, until the code expires or
/// `cancelled` says stop.
pub fn wait_for_sign_in(
    ep: &Endpoints,
    client_id: &str,
    code: &DeviceCode,
    cancelled: &dyn Fn() -> bool,
) -> Result<MsTokens, String> {
    let mut interval = code.interval;
    loop {
        let until = std::time::Instant::now() + Duration::from_secs(interval);
        while std::time::Instant::now() < until {
            if cancelled() {
                return Err("Sign-in cancelled.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if now() > code.expires_at {
            return Err("The sign-in code expired. Start again.".into());
        }
        match poll(ep, client_id, code)? {
            Poll::Pending => {}
            Poll::SlowDown => interval += 5,
            Poll::Done(tokens) => return Ok(tokens),
        }
    }
}

fn refresh_ms(ep: &Endpoints, client_id: &str, refresh: &str) -> Result<MsTokens, String> {
    let (status, v) = send(
        agent().post(&format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh),
            ("scope", SCOPE),
        ])),
        "Microsoft",
    )?;
    ms_tokens(status, &v)
}

// ----- Xbox Live, XSTS, Minecraft ----------------------------------------------------------------

/// The XSTS `XErr` codes, as the official launcher explains them.
fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one, then try again.".into(),
        2148916235 => "Xbox Live is not available in your country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com before it can play.".into(),
        2148916238 => "This is a child account: an adult has to add it to a Microsoft family first.".into(),
        other => format!("Xbox Live refused the sign-in (error {other})."),
    }
}

/// The Minecraft access token from the Microsoft one, through Xbox Live and XSTS.
struct Minecraft {
    token: String,
    expires_at: u64,
    xuid: Option<String>,
}

fn minecraft_login(ep: &Endpoints, ms_access: &str) -> Result<Minecraft, String> {
    let (status, xbl) = send(
        agent().post(&ep.xbox_user),
        Some(Body::Json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    let xbl_token = str_field(&xbl, "Token")
        .filter(|_| status == 200)
        .ok_or("Xbox Live refused the sign-in.")?;
    let (status, xsts) = send(
        agent().post(&ep.xsts),
        Some(Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    if let Some(code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(xsts_error(code));
    }
    let claims = xsts.pointer("/DisplayClaims/xui/0");
    let (Some(xsts_token), Some(uhs)) = (
        str_field(&xsts, "Token").filter(|_| status == 200),
        claims.and_then(|c| str_field(c, "uhs")),
    ) else {
        return Err("Xbox Live refused the sign-in.".into());
    };
    let xuid = claims.and_then(|c| str_field(c, "xid"));
    let (status, mc) = send(
        agent().post(&format!("{}/authentication/login_with_xbox", ep.minecraft)),
        Some(Body::Json(
            json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }),
        )),
        "the Minecraft services",
    )?;
    match (status, str_field(&mc, "access_token")) {
        (200, Some(token)) => Ok(Minecraft {
            token,
            expires_at: now()
                + mc.get("expires_in")
                    .and_then(Value::as_u64)
                    .unwrap_or(86400),
            xuid,
        }),
        // New Azure apps are refused until Mojang approves them (Minecraft API access).
        (403, _) => Err(
            "The Minecraft services refused this launcher: its app is not approved by Mojang yet."
                .into(),
        ),
        (429, _) => Err("Too many sign-ins in a short time. Wait a minute and try again.".into()),
        _ => Err("The Minecraft services refused the sign-in.".into()),
    }
}

fn bearer(request: ureq::Request, token: &str) -> ureq::Request {
    request.set("Authorization", &format!("Bearer {token}"))
}

/// Whether the account owns Minecraft: Java Edition (Game Pass included).
fn owns_game(ep: &Endpoints, token: &str) -> Result<bool, String> {
    let (status, v) = send(
        bearer(
            agent().get(&format!("{}/entitlements/mcstore", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status != 200 {
        return Err("Could not check which games this account owns.".into());
    }
    Ok(v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|i| {
                matches!(
                    i.get("name").and_then(Value::as_str),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        }))
}

/// A Minecraft profile, as the launcher shows and remembers it (no secrets).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Account {
    /// The profile UUID without dashes.
    pub uuid: String,
    pub name: String,
    /// The active skin's URL (textures.minecraft.net) and model ("classic" or "slim").
    #[serde(default)]
    pub skin: Option<(String, String)>,
    #[serde(default)]
    pub cape: Option<String>,
}

fn profile(ep: &Endpoints, token: &str) -> Result<Account, String> {
    let (status, v) = send(
        bearer(
            agent().get(&format!("{}/minecraft/profile", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status == 404 {
        return Err("This account has no Minecraft profile yet. Create one at minecraft.net, then sign in again.".into());
    }
    let (Some(uuid), Some(name)) = (str_field(&v, "id"), str_field(&v, "name")) else {
        return Err("Could not read the Minecraft profile.".into());
    };
    let active = |key: &str| {
        v.get(key)
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE"))
            })
            .cloned()
    };
    Ok(Account {
        uuid: uuid.replace('-', "").to_lowercase(),
        name,
        skin: active("skins").and_then(|s| {
            Some((
                str_field(&s, "url")?,
                str_field(&s, "variant")
                    .unwrap_or_else(|| "CLASSIC".into())
                    .to_lowercase(),
            ))
        }),
        cape: active("capes").and_then(|c| str_field(&c, "url")),
    })
}

/// Whether the signed-in account still owns Minecraft: Java Edition, asked again now: the
/// launcher downloads the game's files only for an owner (sources.rs).
pub fn owns_minecraft(ep: &Endpoints, secrets: &Secrets) -> Result<bool, String> {
    owns_game(ep, &secrets.minecraft_token)
}

/// What the keychain keeps for one account.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Secrets {
    pub(super) refresh_token: String,
    pub(super) minecraft_token: String,
    pub(super) minecraft_expires_at: u64,
    #[serde(default)]
    pub(super) xuid: Option<String>,
}
impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(<hidden>)")
    }
}

/// From Microsoft tokens to a ready account: Xbox, XSTS, Minecraft, ownership, profile.
pub fn finish_sign_in(ep: &Endpoints, ms: MsTokens) -> Result<(Account, Secrets), String> {
    let mc = minecraft_login(ep, &ms.access)?;
    if !owns_game(ep, &mc.token)? {
        return Err("This account doesn't own Minecraft: Java Edition.".into());
    }
    let account = profile(ep, &mc.token)?;
    Ok((
        account,
        Secrets {
            refresh_token: ms.refresh,
            minecraft_token: mc.token,
            minecraft_expires_at: mc.expires_at,
            xuid: mc.xuid,
        },
    ))
}

/// A Minecraft token with at least an hour left, renewed through the refresh token if needed.
/// The profile is read again on renewal (the name may have changed).
pub fn fresh(
    ep: &Endpoints,
    client_id: &str,
    account: &Account,
    secrets: Secrets,
) -> Result<(Account, Secrets), String> {
    if secrets.minecraft_expires_at > now() + RENEW_BEFORE {
        return Ok((account.clone(), secrets));
    }
    let ms = refresh_ms(ep, client_id, &secrets.refresh_token)?;
    let (renewed, secrets) = finish_sign_in(ep, ms)?;
    if renewed.uuid != account.uuid {
        return Err(
            "The Microsoft account now has another Minecraft profile. Sign in again.".into(),
        );
    }
    Ok((renewed, secrets))
}
