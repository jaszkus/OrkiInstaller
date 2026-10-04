#[derive(Debug, Clone, PartialEq)]
pub struct ThemeTokens {
    pub bg: [f32; 4],
    pub surface: [f32; 4],
    pub text: [f32; 4],
    pub accent: [f32; 4],
    pub radius_md: f32,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            bg: [0.05, 0.07, 0.05, 1.0],
            surface: [0.08, 0.11, 0.08, 1.0],
            text: [0.91, 0.94, 0.91, 1.0],
            accent: [0.24, 0.86, 0.52, 1.0],
            radius_md: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeTokens;

    #[test]
    fn default_tokens() {
        let t = ThemeTokens::default();
        assert_eq!(t.radius_md, 10.0);
        assert_eq!(t.accent[3], 1.0);
    }
}
