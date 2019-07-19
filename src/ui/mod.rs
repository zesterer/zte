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
    State,
};

#[derive(Copy, Clone)]
pub struct Context<'a> {
    theme: &'a Theme,
    state: &'a State,
}

pub trait Element {
    fn handle(&mut self, ctx: Context, event: Event);
    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool);
}

pub struct MainUi {
    theme: Theme,
    state: State,
    panels: Panels,
    menu: Option<Menu>,
}

impl MainUi {
    pub fn new(theme: Theme, state: State) -> Self {
        Self {
            theme,
            state,
            panels: Panels::empty(3),
            menu: None,
        }
    }

    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    pub fn handle(&mut self, event: Event) {
        match event {
            Event::OpenPrompt => unimplemented!(),
            Event::OpenSwitcher => unimplemented!(),
            event => self.panels.handle(
                Context {
                    theme: &self.theme,
                    state: &self.state,
                },
                event,
            ),
        }
    }

    pub fn render(&self, canvas: &mut impl Canvas) {
        let ctx = Context {
            theme: &self.theme,
            state: &self.state,
        };

        self.panels.render(ctx, canvas, true);
    }
}

impl Default for MainUi {
    fn default() -> Self {
        Self::new(Theme::default(), State::default())
    }
}

pub enum Menu {}
