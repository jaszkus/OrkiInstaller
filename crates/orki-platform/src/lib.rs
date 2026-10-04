use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    pub install_dir: PathBuf,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsInfo {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub is_server: bool,
}

impl OsInfo {
    pub fn version_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.build)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PlatformError {
    #[error("elevation required")]
    ElevationRequired,
    #[error("platform op failed: {0}")]
    Failed(String),
    #[error("io: {0}")]
    Io(String),
}

pub trait Registry: Send + Sync {
    fn get_string(
        &self,
        hive: Scope,
        key: &str,
        value: &str,
    ) -> Result<Option<String>, PlatformError>;
    fn set_string(
        &self,
        hive: Scope,
        key: &str,
        value: &str,
        data: &str,
    ) -> Result<(), PlatformError>;
    fn remove(&self, hive: Scope, key: &str) -> Result<(), PlatformError>;
}

pub trait Shortcuts: Send + Sync {
    fn create(
        &self,
        location: ShortcutLocation,
        name: &str,
        target: &Path,
    ) -> Result<(), PlatformError>;
    fn remove(&self, location: ShortcutLocation, name: &str) -> Result<(), PlatformError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutLocation {
    StartMenu,
    Desktop,
}

pub trait Platform: Send + Sync {
    fn os_info(&self) -> OsInfo;
    fn app_locations(&self, scope: Scope, app_name: &str) -> Locations;
    fn registry(&self) -> &dyn Registry;
    fn shortcuts(&self) -> &dyn Shortcuts;
}

#[cfg(test)]
mod tests {
    use super::{OsInfo, Scope};

    #[test]
    fn os_version_format() {
        let os = OsInfo {
            major: 10,
            minor: 0,
            build: 26100,
            is_server: false,
        };
        assert_eq!(os.version_string(), "10.0.26100");
    }

    #[test]
    fn scope_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Scope>();
    }
}
