// game-module-cfg: target_os = "horizon"
// game-module-name: native_config_files
use super::Game;
use mod_api::{ConfigEntry, ConfigFileError, ConfigFiles, ConfigPath, ConfigStat};

struct Unavailable;

impl ConfigFiles for Unavailable {
    fn read(&self, _: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
        Err(ConfigFileError::NoCallingMod)
    }
    fn write(&self, _: &ConfigPath, _: &[u8]) -> Result<(), ConfigFileError> {
        Err(ConfigFileError::NoCallingMod)
    }
    fn list(
        &self,
        _: &ConfigPath,
        _: Option<&str>,
        _: u32,
    ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
        Err(ConfigFileError::NoCallingMod)
    }
    fn stat(&self, _: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
        Err(ConfigFileError::NoCallingMod)
    }
}

impl Game<'_> {
    pub(super) fn config_files() -> &'static dyn ConfigFiles {
        &Unavailable
    }
    pub(super) fn data_files() -> &'static dyn ConfigFiles {
        &Unavailable
    }
    pub(super) fn world_data_files(&self) -> &dyn ConfigFiles {
        &Unavailable
    }
    pub(super) fn is_loaded(id: &str) -> bool {
        mod_api::__private::loaded_mods().get(id).is_some()
    }
}
