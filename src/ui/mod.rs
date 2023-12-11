mod theme;
mod editor;
mod terminal;
mod panels;
mod switcher;
mod confirm;
mod opener;
mod prompt;

// Reexports
pub use self::{
    theme::Theme,
    editor::Editor,
    terminal::Terminal,
    panels::{Panels, Tile},
    switcher::Switcher,
    confirm::Confirm,
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
    pub fn new(theme: Theme, state: State, buffers: Vec<BufferId>) -> Self {
        let mut ctx = Context {
            theme,
            state,
            active_buffer: BufferId(0), // Gets replaced later
            secondary_events: VecDeque::new(),
        };

        let panels = match buffers.len() {
            0 => Panels::empty(&mut ctx, 1),
            _ => {
                let mut panels = Panels::empty(&mut ctx, 0);

                for buffer in buffers.into_iter().rev() {
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
        if let Err(event) = match &mut self.menu {
            Some(menu) => match event {
                Event::CloseMenu => Ok(self.menu = None),
                Event::Escape => match self.menu.take() {
                    Some(Menu::Switcher(switcher)) => Ok(switcher.cancel(&mut self.ctx)),
                    Some(Menu::Confirm(confirm)) => Ok(confirm.cancel(&mut self.ctx)),
                    Some(Menu::Opener(_)) => Ok(()),
                    None => Err(event),
                },
                event => match menu {
                    Menu::Switcher(switcher) => switcher.handle(&mut self.ctx, event),
                    Menu::Opener(opener) => opener.handle(&mut self.ctx, event),
                    Menu::Confirm(confirm) => confirm.handle(&mut self.ctx, event),
                },
            },
            None => Err(event)
        } {
            match event {
                Event::Quit => return true,
                Event::Escape if !self.ctx.state.any_unsaved() => return true,
                Event::Escape => self.menu = Some(Menu::Confirm(Confirm::quit(&mut self.ctx))),
                Event::OpenPrompt => unimplemented!(),
                Event::OpenSwitcher => match self.panels.active_mut().and_then(|col| col.active_mut()) {
                    Some(Tile::Editor(editor)) => self.menu = Some(Menu::Switcher(Switcher::new(
                        &mut self.ctx,
                        editor.buffer().clone(),
                    ))),
                    _ => {},
                },
                Event::OpenOpener => self.menu = Some(Menu::Opener(Opener::new(&mut self.ctx))),
                event => self.panels.handle(&mut self.ctx, event),
            }
        }

        if let Some(e) = self.ctx.secondary_events.pop_front() {
            return self.handle(e);
        }

        false
    }

    pub fn update(&mut self, canvas: &mut impl Canvas) {
        self.panels.update(&mut self.ctx, canvas, self.menu.is_none());

        match &mut self.menu {
            Some(Menu::Switcher(switcher)) => switcher.update(&mut self.ctx, canvas, true),
            Some(Menu::Opener(opener)) => opener.update(&mut self.ctx, canvas, true),
            Some(Menu::Confirm(confirm)) => confirm.update(&mut self.ctx, canvas, true),
            None => {},
        }
    }

    pub fn render(&mut self, canvas: &mut impl Canvas) {
        self.panels.render(&mut self.ctx, canvas, true);

        match &self.menu {
            Some(Menu::Switcher(switcher)) => switcher.render(&mut self.ctx, canvas, true),
            Some(Menu::Opener(opener)) => opener.render(&mut self.ctx, canvas, true),
            Some(Menu::Confirm(confirm)) => confirm.render(&mut self.ctx, canvas, true),
            None => {},
        }
    }
}

pub enum Menu {
    Switcher(Switcher),
    Opener(Opener),
    Confirm(Confirm),
}
