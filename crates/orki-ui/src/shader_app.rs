use iced::widget::{button, column, container, progress_bar, text};
use iced::{Alignment, Element, Task, Theme};

use crate::nav::{Navigator, PageId};
use crate::widget::aurora;

#[derive(Debug, Clone)]
pub struct SpikeApp {
    nav: Navigator,
    progress: f32,
    started: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Next,
    Prev,
    Install,
    Tick,
}

impl Default for SpikeApp {
    fn default() -> Self {
        Self {
            nav: Navigator::standard(),
            progress: 0.0,
            started: std::time::Instant::now(),
        }
    }
}

impl SpikeApp {
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Next => {
                self.nav.go_next();
                Task::none()
            }
            Msg::Prev => {
                self.nav.go_prev();
                Task::none()
            }
            Msg::Install => Task::done(Msg::Tick),
            Msg::Tick => {
                self.progress =
                    ((self.started.elapsed().as_secs_f32() / 2.0) % 1.0).clamp(0.0, 1.0);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Msg> {
        let page = self.nav.current().map(|p| p.id).unwrap_or(PageId::Welcome);

        let content: Element<'_, Msg> = match page {
            PageId::Progress => column![
                text("Installing...").size(24),
                progress_bar(0.0..=1.0, self.progress),
                button("Simulate").on_press(Msg::Install),
            ]
            .spacing(16)
            .into(),
            PageId::Finish => column![text("Done!").size(28), button("Back").on_press(Msg::Prev),]
                .spacing(16)
                .into(),
            _ => column![
                text("OrkiInstaller — spike").size(22),
                text("iced + wgpu + WGSL shader background").size(14),
                button("Next").on_press(Msg::Next),
            ]
            .spacing(16)
            .into(),
        };

        let page_label = self
            .nav
            .current()
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let ui = column![text(page_label).size(12), content,]
            .spacing(24)
            .align_x(Alignment::Center);

        container(column![aurora(self.progress).height(140), ui,].spacing(12))
            .width(iced::Fill)
            .height(iced::Fill)
            .padding(24)
            .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

pub fn run() -> Result<(), iced::Error> {
    iced::application(SpikeApp::default, SpikeApp::update, SpikeApp::view)
        .title(|_: &SpikeApp| String::from("Orki"))
        .theme(SpikeApp::theme)
        .run()
}

#[cfg(test)]
mod tests {
    use super::{Msg, SpikeApp};
    use crate::nav::PageId;

    #[test]
    fn update_nav() {
        let mut app = SpikeApp::default();
        let _ = app.update(Msg::Next);
        assert_eq!(app.nav.current().map(|p| p.id), Some(PageId::License));
        let _ = app.update(Msg::Prev);
        assert_eq!(app.nav.current().map(|p| p.id), Some(PageId::Welcome));
    }
}
