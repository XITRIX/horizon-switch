//! The game-owned, side-effect-free startup validation boundary.
use super::*;
use crate::entry::startup_path;

impl Options {
    /// The complete game-owned command-line boundary, before leases or native hooks.
    /// A bad command line is a [`launch_api::UsageError`] (exit code 2, no crash report).
    pub(crate) fn validate(launch: LaunchArguments) -> Result<Option<Self>> {
        let Some(mut o) =
            Self::from_launch(launch).map_err(|e| launch_api::UsageError(e.to_string()))?
        else {
            return Ok(None);
        };
        // Paths given on the command line stay relative to where the game was started.
        if o.game_dir.is_some() {
            o.control = o.control.as_deref().map(startup_path::absolute).transpose()?;
            o.resources = startup_path::absolute(&o.resources)?;
            o.report = o.report.as_deref().map(startup_path::absolute).transpose()?;
        }
        // Refused here, before the account's file is used up or anything loads.
        if let Some(path) = &o.control {
            horizon_client::control::check_path(path).map_err(|e| {
                launch_api::UsageError(format!("--control-socket {}: {e}", path.display()))
            })?;
        }
        Ok(Some(o))
    }
}
