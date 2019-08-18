mod theme;
mod editor;
mod panels;
mod switcher;

// Reexports
pub use self::{
    theme::Theme,
    editor::Editor,
    panels::{Panels, Tile},
    switcher::Switcher,
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
    type Response = ();

    fn handle(&mut self, ctx: Context, event: Event) -> Self::Response;
    fn update(&mut self, ctx: Context, canvas: &mut impl Canvas, active: bool);
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
        let panels = match state.buffers().len() {
            0 => Panels::empty(1),
            _ => state
                .buffers()
                .iter()
                .rev()
                .fold(Panels::empty(0), |mut panels, buffer| {
                    panels.insert_column(0, Tile::from(buffer.clone()));
                    panels
                }),
        };

        Self {
            theme,
            state,
            panels,
            menu: None,
        }
    }

    pub fn handle(&mut self, event: Event) {
        match &mut self.menu {
            Some(menu) => match event {
                Event::Escape => self.menu = None,
                _ => { /* TODO: Send event to menu here */ },
            },
            None => match event {
                Event::OpenPrompt => unimplemented!(),
                Event::OpenSwitcher => self.menu = Some(Menu::Switcher(Switcher::default())),
                event => self.panels.handle(
                    Context {
                        theme: &self.theme,
                        state: &self.state,
                    },
                    event,
                ),
            }
        }
    }

    pub fn update(&mut self, canvas: &mut impl Canvas) {
        let ctx = Context {
            theme: &self.theme,
            state: &self.state,
        };

        self.panels.update(ctx, canvas, true);

        match &mut self.menu {
            Some(Menu::Switcher(switcher)) => switcher.update(ctx, canvas, true),
            None => {},
        }
    }

    pub fn render(&self, canvas: &mut impl Canvas) {
        let ctx = Context {
            theme: &self.theme,
            state: &self.state,
        };

        self.panels.render(ctx, canvas, true);

        match &self.menu {
            Some(Menu::Switcher(switcher)) => switcher.render(ctx, canvas, true),
            None => {},
        }
    }
}

impl Default for MainUi {
    fn default() -> Self {
        Self::new(Theme::default(), State::default())
    }
}

pub enum Menu {
    Switcher(Switcher),
}
