#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCaps {
    pub fs_install_dir: bool,
    pub registry_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub caps: PluginCaps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Loaded,
    Running,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::{PluginCaps, PluginManifest};

    #[test]
    fn manifest_shape() {
        let p = PluginManifest {
            name: "my-plugin".into(),
            version: "1.0.0".into(),
            caps: PluginCaps {
                fs_install_dir: true,
                registry_prefixes: vec!["HKCU/Software/Example".into()],
            },
        };
        assert_eq!(p.name, "my-plugin");
        assert!(p.caps.fs_install_dir);
    }
}
