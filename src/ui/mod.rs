mod theme;
mod editor;
mod terminal;
mod panels;
mod switcher;
mod opener;
mod prompt;

// Reexports
pub use self::{
    theme::Theme,
    editor::Editor,
    terminal::Terminal,
    panels::{Panels, Tile},
    switcher::Switcher,
    opener::Opener,
    prompt::Prompt,
};

use std::collections::VecDeque;
use crate::{
    Canvas,
    Event,
    State,
    buffer::BufferId,
};

pub struct Context {
    theme: Theme,
    state: State,
    active_buffer: BufferId,
    secondary_events: VecDeque<Event>,
}

pub trait Element {
    type Response;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response;
    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool);
    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool);
}

pub struct MainUi {
    ctx: Context,
    panels: Panels,
    menu: Option<Menu>,
}

impl MainUi {
    pub fn new(theme: Theme, state: State) -> Self {
        let mut ctx = Context {
            theme,
            state,
            active_buffer: BufferId(0), // Gets replaced later
            secondary_events: VecDeque::new(),
        };

        let buffer_count = ctx.state.buffers().len();
        let panels = match buffer_count {
            0 => Panels::empty(&mut ctx, 1),
            _ => {
                let mut panels = Panels::empty(&mut ctx, 0);

                for buffer in ctx.state
                    .buffers()
                    .collect::<Vec<_>>()
                {
                    panels.insert_column(0, Tile::Editor(Editor::from(ctx.state
                        .new_handle(buffer)
                        .unwrap())));
                }

                panels
            },
        };

        assert!(ctx.state.buffers().len() != 0);

        Self {
            ctx,
            panels,
            menu: None,
        }
    }

    pub fn handle(&mut self, event: Event) -> bool {
        match &mut self.menu {
            Some(menu) => match event {
                Event::Escape | Event::CloseMenu => self.menu = None,
                event => match menu {
                    Menu::Switcher(switcher) => switcher.handle(&mut self.ctx, event),
                    Menu::Opener(opener) => opener.handle(&mut self.ctx, event),
                },
            },
            None => match event {
                Event::Escape => return true,
                Event::OpenPrompt => unimplemented!(),
                Event::OpenSwitcher => self.menu = Some(Menu::Switcher(Switcher::new(&mut self.ctx))),
                Event::OpenOpener => self.menu = Some(Menu::Opener(Opener::new(&mut self.ctx))),
                event => self.panels.handle(&mut self.ctx, event),
            }
        }

        if let Some(e) = self.ctx.secondary_events.pop_front() {
            self.handle(e);
        }

        false
    }

    pub fn update(&mut self, canvas: &mut impl Canvas) {
        self.panels.update(&mut self.ctx, canvas, true);

        match &mut self.menu {
            Some(Menu::Switcher(switcher)) => switcher.update(&mut self.ctx, canvas, true),
            Some(Menu::Opener(opener)) => opener.update(&mut self.ctx, canvas, true),
            None => {},
        }
    }

    pub fn render(&mut self, canvas: &mut impl Canvas) {
        self.panels.render(&mut self.ctx, canvas, true);

        match &self.menu {
            Some(Menu::Switcher(switcher)) => switcher.render(&mut self.ctx, canvas, true),
            Some(Menu::Opener(opener)) => opener.render(&mut self.ctx, canvas, true),
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
    Opener(Opener),
}
