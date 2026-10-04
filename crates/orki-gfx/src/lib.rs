#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrkiUniforms {
    pub resolution: [f32; 2],
    pub time: f32,
    pub delta: f32,
    pub mouse: [f32; 4],
    pub progress: f32,
    pub phase: f32,
    pub scale: f32,
    pub dark: f32,
    pub palette: [[f32; 4]; 8],
    pub params: [[f32; 4]; 8],
}

pub const ORKI_UNIFORMS_SIZE: usize = size_of::<OrkiUniforms>();

impl Default for OrkiUniforms {
    fn default() -> Self {
        Self {
            resolution: [1280.0, 720.0],
            time: 0.0,
            delta: 1.0 / 60.0,
            mouse: [0.0; 4],
            progress: 0.0,
            phase: 0.0,
            scale: 1.0,
            dark: 1.0,
            palette: [[0.0; 4]; 8],
            params: [[0.0; 4]; 8],
        }
    }
}

impl OrkiUniforms {
    pub fn set_palette_color(&mut self, index: usize, rgba: [f32; 4]) {
        if index < 8 {
            self.palette[index] = rgba;
        }
    }

    pub fn set_param(&mut self, index: usize, value: [f32; 4]) {
        if index < 8 {
            self.params[index] = value;
        }
    }
}

pub const PRESETS: [&str; 8] = [
    "aurora",
    "mesh-gradient",
    "starfield",
    "ripple",
    "glass-blur",
    "noise-flow",
    "crt",
    "glow-bar",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Auto,
    Low,
    High,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GfxBackend {
    Dx12,
    Vulkan,
    OpenGl,
    TinySkia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInfo {
    pub backend: GfxBackend,
    pub software: bool,
    pub device_name: String,
}

impl AdapterInfo {
    pub fn prefers_static_fallback(&self) -> bool {
        self.software
            || self
                .device_name
                .to_ascii_lowercase()
                .contains("basic render driver")
            || self.device_name.to_ascii_lowercase().contains("warp")
            || matches!(self.backend, GfxBackend::TinySkia)
    }
}

pub const FULLSCREEN_TRIANGLE: [[f32; 2]; 3] = [[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]];

#[cfg(test)]
mod tests {
    use super::{AdapterInfo, GfxBackend, ORKI_UNIFORMS_SIZE, OrkiUniforms, PRESETS, Quality};

    #[test]
    fn uniforms_layout_stable() {
        assert_eq!(ORKI_UNIFORMS_SIZE, 304);
        let mut u = OrkiUniforms::default();
        u.set_palette_color(0, [0.1, 0.2, 0.3, 1.0]);
        u.set_param(3, [0.5, 0.0, 0.0, 0.0]);
        assert_eq!(u.palette[0], [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(u.params[3][0], 0.5);
        u.set_palette_color(9, [1.0; 4]);
        assert_eq!(u.palette[9 - 2], [0.0; 4]);
    }

    #[test]
    fn presets_catalog() {
        assert_eq!(PRESETS.len(), 8);
        assert!(PRESETS.contains(&"glow-bar"));
        assert_eq!(Quality::Auto, Quality::Auto);
    }

    #[test]
    fn software_adapter_prefers_fallback() {
        let warp = AdapterInfo {
            backend: GfxBackend::Dx12,
            software: true,
            device_name: "Microsoft Basic Render Driver".into(),
        };
        let hw = AdapterInfo {
            backend: GfxBackend::Dx12,
            software: false,
            device_name: "NVIDIA GeForce RTX 4070".into(),
        };
        let tiny = AdapterInfo {
            backend: GfxBackend::TinySkia,
            software: false,
            device_name: "tiny-skia".into(),
        };
        assert!(warp.prefers_static_fallback());
        assert!(tiny.prefers_static_fallback());
        assert!(!hw.prefers_static_fallback());
    }
}
