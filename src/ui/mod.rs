mod theme;
mod editor;
mod panels;

// Reexports
pub use self::{
    theme::Theme,
    editor::Editor,
    panels::Panels,
};

use crate::{
    Canvas,
    Event,
};

#[derive(Copy, Clone)]
pub struct Context<'a> {
    theme: &'a Theme,
}

pub trait Element {
    fn update(&mut self, ctx: Context) {}
    fn handle(&mut self, ctx: Context, event: Event);
    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool);
}

pub struct MainUi {
    theme: Theme,
    panels: Panels,
}

impl MainUi {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            panels: Panels::empty(3),
        }
    }

    pub fn handle(&mut self, event: Event) {
        let ctx = Context {
            theme: &self.theme,
        };

        self.panels.handle(ctx, event);
    }

    pub fn render(&self, canvas: &mut impl Canvas) {
        let ctx = Context {
            theme: &self.theme,
        };

        self.panels.render(ctx, canvas, true);
    }
}

impl Default for MainUi {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}
