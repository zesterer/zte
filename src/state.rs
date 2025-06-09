use crate::{Args, Dir, Error, theme};
use slotmap::{HopSlotMap, new_key_type};
use std::{io, ops::Range, path::PathBuf};

new_key_type! {
    pub struct BufferId;
    pub struct CursorId;
}

#[derive(Copy, Clone, Default)]
pub struct Cursor {
    pub base: usize,
    pub pos: usize,
    // Used to 'remember' the desired column when skipping over shorter lines
    desired_col: isize,
}

impl Cursor {
    fn reset_desired_col(&mut self, text: &Text) {
        self.desired_col = text.to_coord(self.pos)[0];
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        if self.base == self.pos {
            None
        } else {
            Some(self.base.min(self.pos)..self.base.max(self.pos))
        }
    }
}

#[derive(Default)]
pub struct Text {
    chars: Vec<char>,
}

impl ToString for Text {
    fn to_string(&self) -> String {
        self.chars.iter().copied().collect()
    }
}

impl Text {
    pub fn to_coord(&self, pos: usize) -> [isize; 2] {
        let mut n = 0;
        let mut last_n = 0;
        let mut i: usize = 0;
        for line in self.lines() {
            last_n = n;
            i += 1;
            if (n..n + line.len()).contains(&pos) {
                break;
            }
            n += line.len();
        }
        [(pos - last_n) as isize, i.saturating_sub(1) as isize]
    }

    pub fn to_pos(&self, coord: [isize; 2]) -> usize {
        if coord[1] < 0 {
            return 0;
        }
        let mut pos = 0;
        for (i, line) in self.lines().enumerate() {
            if i as isize == coord[1] {
                return pos + coord[0].clamp(0, line.len().saturating_sub(1) as isize) as usize;
            } else {
                pos += line.len();
            }
        }
        pos.min(self.chars.len())
    }

    /// Return an iterator over the lines of the text.
    ///
    /// Guarantees:
    /// - If you sum the lengths of each line, it will be the same as the length (in characters) of the text
    pub fn lines(&self) -> impl Iterator<Item = &[char]> {
        let mut start = 0;
        let mut i = 0;
        let mut finished = false;
        core::iter::from_fn(move || {
            loop {
                let Some(c) = self.chars.get(i) else {
                    return if finished {
                        None
                    } else {
                        let line = &self.chars[start..];
                        finished = true;
                        Some(line)
                    };
                };
                i += 1;
                if *c == '\n' {
                    let line = &self.chars[start..i];
                    start = i;
                    return Some(line);
                }
            }
        })
    }
}

#[derive(Default)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub text: Text,
    pub cursors: HopSlotMap<CursorId, Cursor>,
}

impl Buffer {
    pub fn from_file(path: PathBuf) -> Result<Self, Error> {
        let chars = match std::fs::read_to_string(&path) {
            Ok(s) => s.chars().collect(),
            // If the file doesn't exist, create a new file
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            path: Some(path),
            text: Text { chars },
            cursors: HopSlotMap::default(),
        })
    }

    pub fn clear(&mut self) {
        self.text.chars.clear();
        // Reset cursors
        self.cursors.values_mut().for_each(|cursor| {
            *cursor = Cursor::default();
        });
    }

    pub fn move_cursor(
        &mut self,
        cursor_id: CursorId,
        dir: Dir,
        dist: [usize; 2],
        retain_base: bool,
    ) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        match dir {
            Dir::Left => {
                cursor.pos = cursor.pos.saturating_sub(dist[0]);
                cursor.reset_desired_col(&self.text);
            }
            Dir::Right => {
                cursor.pos = (cursor.pos + dist[0]).min(self.text.chars.len());
                cursor.reset_desired_col(&self.text);
            }
            Dir::Up => {
                let coord = self.text.to_coord(cursor.pos);
                // Special case: pressing 'up' at the top of the screen resets the cursor to the beginning
                if coord[1] <= 0 {
                    cursor.pos = 0;
                    cursor.reset_desired_col(&self.text);
                } else {
                    cursor.pos = self
                        .text
                        .to_pos([cursor.desired_col, coord[1] - dist[1] as isize]);
                }
            }
            Dir::Down => {
                let coord = self.text.to_coord(cursor.pos);
                cursor.pos = self
                    .text
                    .to_pos([cursor.desired_col, coord[1] + dist[1] as isize]);
            }
        };

        if !retain_base {
            cursor.base = cursor.pos;
        }
    }

    pub fn insert(&mut self, pos: usize, c: char) {
        self.text.chars.insert(pos.min(self.text.chars.len()), c);
        self.cursors.values_mut().for_each(|cursor| {
            if cursor.base >= pos {
                cursor.base += 1;
            }
            if cursor.pos >= pos {
                cursor.pos += 1;
                cursor.reset_desired_col(&self.text);
            }
        });
    }

    pub fn enter(&mut self, cursor_id: CursorId, c: char) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
            self.enter(cursor_id, c);
        } else {
            self.insert(cursor.pos, c);
        }
    }

    // Assumes range is well-formed
    pub fn remove(&mut self, range: Range<usize>) {
        // TODO: Bell if false?
        self.text.chars.drain(range.clone());
        self.cursors.values_mut().for_each(|cursor| {
            if cursor.base >= range.start {
                cursor.base = cursor
                    .base
                    .saturating_sub(range.end - range.start)
                    .max(range.start);
            }
            if cursor.pos >= range.start {
                cursor.pos = cursor
                    .pos
                    .saturating_sub(range.end - range.start)
                    .max(range.start);
                cursor.reset_desired_col(&self.text);
            }
        });
    }

    pub fn backspace(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else {
            if let Some(pos) = cursor.pos.checked_sub(1) {
                self.remove(pos..pos + 1);
            }
        }
    }

    pub fn delete(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else {
            self.remove(cursor.pos..cursor.pos + 1);
        }
    }

    pub fn start_session(&mut self) -> CursorId {
        self.cursors.insert(Cursor::default())
    }

    pub fn end_session(&mut self, cursor_id: CursorId) {
        self.cursors.remove(cursor_id);
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
            this.buffers.insert(Buffer::from_file(path)?);
        }

        Ok(this)
    }
}

impl State {
    pub fn tick(&mut self) {
        self.tick += 1;
    }
}
