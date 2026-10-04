#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageId {
    Welcome,
    License,
    Components,
    Location,
    Ready,
    Progress,
    Finish,
    Error,
    Maintenance,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub id: PageId,
    pub title: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigator {
    pages: Vec<Page>,
    current: usize,
}

impl Navigator {
    pub fn new(pages: Vec<Page>) -> Self {
        Self { pages, current: 0 }
    }

    pub fn standard() -> Self {
        let ids = [
            (PageId::Welcome, "Welcome"),
            (PageId::License, "License"),
            (PageId::Components, "Components"),
            (PageId::Location, "Location"),
            (PageId::Ready, "Ready"),
            (PageId::Progress, "Progress"),
            (PageId::Finish, "Finish"),
        ];
        Self::new(
            ids.into_iter()
                .map(|(id, title)| Page {
                    id,
                    title: title.to_string(),
                    enabled: true,
                })
                .collect(),
        )
    }

    pub fn current(&self) -> Option<&Page> {
        self.pages.get(self.current)
    }

    pub fn go_next(&mut self) -> bool {
        if self.current + 1 < self.pages.len() {
            self.current += 1;
            true
        } else {
            false
        }
    }

    pub fn go_prev(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            true
        } else {
            false
        }
    }
}

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
    use super::{Navigator, PageId};

    #[test]
    fn navigation_walk() {
        let mut nav = Navigator::standard();
        assert_eq!(nav.current().map(|p| p.id), Some(PageId::Welcome));
        assert!(nav.go_next());
        assert!(nav.go_next());
        assert!(nav.go_prev());
        assert_eq!(nav.current().map(|p| p.id), Some(PageId::License));
    }

    #[test]
    fn navigation_bounds() {
        let mut nav = Navigator::standard();
        while nav.go_next() {}
        assert_eq!(nav.current().map(|p| p.id), Some(PageId::Finish));
        assert!(!nav.go_next());
        assert!(nav.go_prev());
        while nav.go_prev() {}
        assert_eq!(nav.current().map(|p| p.id), Some(PageId::Welcome));
        assert!(!nav.go_prev());
    }
}
