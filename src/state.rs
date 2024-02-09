use crate::{
    ui::{self, Resp, Element as _},
    Action, Event, Args, Error, Color,
};
use crossterm::event::{KeyCode, KeyModifiers, KeyEvent, KeyEventKind};
use slotmap::{HopSlotMap, new_key_type};
use std::collections::HashMap;

new_key_type! {
    // Per-activity
    pub struct ViewId;
    pub struct ActivityId;
}

pub struct Cursor {
    base: (usize, usize),
    pos: (usize, usize),
}

pub struct FileView {
    line: usize,
    cursor: Cursor,
    // For searches
    // view_cursor: Option<Cursor>,
}

pub struct File {
    views: HopSlotMap<ViewId, FileView>,
}

pub struct ConsoleView {
    line: usize,
}

pub struct Console {
    views: HopSlotMap<ViewId, ConsoleView>,
}

pub enum Activity {
    File(File),
    Console(Console),
}

pub struct Theme {
    pub ui_bg: Color,
    pub status_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui_bg: Color::AnsiValue(235),
            status_bg: Color::AnsiValue(23),
        }
    }
}

pub struct State {
    pub activities: HopSlotMap<ActivityId, Activity>,
    pub tick: u64,
    pub theme: Theme,
}

impl TryFrom<Args> for State {
    type Error = Error;
    fn try_from(args: Args) -> Result<Self, Self::Error> {
        Ok(Self {
            activities: HopSlotMap::default(),
            tick: 0,
            theme: Theme::default(),
        })
    }
}

impl State {    
    pub fn tick(&mut self) {
        self.tick += 1;
    }
}
