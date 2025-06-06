use crate::{
    Action, Args, Color, Dir, Error, Event, theme,
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
    // Used to 'remember' the desired column when skipping over shorter lines
    desired_col: isize,
}

impl Cursor {
    fn reset_desired_col(&mut self, text: &Text) {
        self.desired_col = text.to_coord(self.pos)[0];
    }
}

pub struct Text {
    chars: Vec<char>,
}

impl Text {
    pub fn to_coord(&self, pos: usize) -> [isize; 2] {
        let mut n = 0;
        let mut i = 0;
        for line in self.lines() {
            if (n..n + line.len() + 1).contains(&pos) {
                return [(pos - n) as isize, i as isize];
            } else {
                n += line.len() + 1;
                i += 1;
            }
        }
        [0, i as isize]
    }

    pub fn to_pos(&self, mut coord: [isize; 2]) -> usize {
        if coord[1] < 0 {
            return 0;
        }
        let mut pos = 0;
        for (i, line) in self.lines().enumerate() {
            if i as isize == coord[1] {
                return pos + coord[0].clamp(0, line.len() as isize) as usize;
            } else {
                pos += line.len() + 1;
            }
        }
        pos.min(self.chars.len())
    }

    pub fn lines(&self) -> impl Iterator<Item = &[char]> {
        self.chars.split(|c| *c == '\n')
    }
}

pub struct Buffer {
    pub path: PathBuf,
    pub text: Text,
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
            text: Text { chars },
            cursors: HopSlotMap::default(),
        })
    }

    pub fn move_cursor(&mut self, cursor_id: CursorId, dir: Dir) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        match dir {
            Dir::Left => {
                cursor.pos = cursor.pos.saturating_sub(1);
                cursor.reset_desired_col(&self.text);
            }
            Dir::Right => {
                cursor.pos = (cursor.pos + 1).min(self.text.chars.len());
                cursor.reset_desired_col(&self.text);
            }
            Dir::Up => {
                let mut coord = self.text.to_coord(cursor.pos);
                cursor.pos = self.text.to_pos([cursor.desired_col, coord[1] - 1]);
            }
            Dir::Down => {
                let mut coord = self.text.to_coord(cursor.pos);
                cursor.pos = self.text.to_pos([cursor.desired_col, coord[1] + 1]);
            }
        };
    }

    pub fn insert(&mut self, pos: usize, c: char) {
        self.text.chars.insert(pos.min(self.text.chars.len()), c);
        self.cursors.values_mut().for_each(|cursor| {
            if cursor.pos >= pos {
                cursor.pos += 1;
                cursor.reset_desired_col(&self.text);
            }
        });
    }

    pub fn remove(&mut self, pos: usize) {
        // TODO: Bell if false?
        if self.text.chars.len() > pos {
            self.text.chars.remove(pos);
            self.cursors.values_mut().for_each(|cursor| {
                if cursor.pos >= pos {
                    cursor.pos = cursor.pos.saturating_sub(1);
                    cursor.reset_desired_col(&self.text);
                }
            });
        }
    }

    pub fn backspace(&mut self, pos: usize) {
        if let Some(pos) = pos.checked_sub(1) {
            self.remove(pos);
        }
    }

    pub fn delete(&mut self, pos: usize) {
        self.remove(pos);
    }

    pub fn start_session(&mut self) -> CursorId {
        self.cursors.insert(Cursor::default())
    }

    pub fn end_session(&mut self, cursor: CursorId) {
        self.cursors.remove(cursor);
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
