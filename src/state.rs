use crate::{
    Action, Args, Color, Error, Event, theme,
    ui::{self, Element as _, Resp},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use slotmap::{HopSlotMap, new_key_type};
use std::{io, path::PathBuf};

new_key_type! {
    pub struct BufferId;
    pub struct CursorId;
}

#[derive(Copy, Clone, Default)]
pub struct Cursor {
    pub pos: usize,
}

pub struct Buffer {
    pub path: PathBuf,
    pub chars: Vec<char>,
    pub cursors: HopSlotMap<CursorId, Cursor>,
}

impl Buffer {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let chars = match std::fs::read_to_string(&path) {
            Ok(s) => s.chars().collect(),
            // If the file doesn't exist, create a new file
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            path,
            chars,
            cursors: HopSlotMap::default(),
        })
    }

    pub fn insert(&mut self, pos: usize, c: char) {
        self.chars.insert(pos, c);
        self.cursors.values_mut().for_each(|c| {
            if c.pos >= pos {
                c.pos += 1
            }
        });
    }
}

pub struct State {
    pub buffers: HopSlotMap<BufferId, Buffer>,
    pub tick: u64,
    pub theme: theme::Theme,
}

impl TryFrom<Args> for State {
    type Error = Error;
    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let mut this = Self {
            buffers: HopSlotMap::default(),
            tick: 0,
            theme: theme::Theme::default(),
        };

        for path in args.paths {
            this.buffers.insert(Buffer::new(path)?);
        }

        Ok(this)
    }
}

impl State {
    pub fn tick(&mut self) {
        self.tick += 1;
    }
}
