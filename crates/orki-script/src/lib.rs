#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxLimits {
    pub max_operations: u64,
    pub max_memory_bytes: u64,
    pub max_execution_ms: u64,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_operations: 1_000_000,
            max_memory_bytes: 32 * 1024 * 1024,
            max_execution_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptCapability {
    Fs,
    Registry,
    Process,
    Ui,
    Log,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEngine {
    Rhai,
    Wasm,
}

#[cfg(test)]
mod tests {
    use super::{SandboxLimits, ScriptCapability};

    #[test]
    fn defaults() {
        let l = SandboxLimits::default();
        assert_eq!(l.max_operations, 1_000_000);
        assert_eq!(l.max_execution_ms, 10_000);
    }

    #[test]
    fn capabilities_copyable() {
        let caps = [ScriptCapability::Fs, ScriptCapability::Log];
        assert_eq!(caps.len(), 2);
    }
}
