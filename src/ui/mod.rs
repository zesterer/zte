mod theme;
mod editor;
mod panels;

use self::{
    theme::Theme,
    editor::Editor,
    panels::Panels,
};
use crate::{
    Canvas,
};

#[derive(Copy, Clone)]
struct Context<'a> {
    theme: &'a Theme,
}

trait Element {
    fn render(&self, ctx: Context, canvas: &mut impl Canvas);
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

    pub fn render(&self, canvas: &mut impl Canvas) {
        let ctx = Context {
            theme: &self.theme,
        };

        Element::render(self, ctx, canvas);
    }
}

impl Default for MainUi {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl Element for MainUi {
    fn render(&self, ctx: Context, canvas: &mut impl Canvas) {
        self.panels.render(ctx, canvas);
    }
}
