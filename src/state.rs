use crate::{
    Args, Dir, Error,
    highlight::{Highlighter, Highlights},
    theme,
};
use slotmap::{HopSlotMap, new_key_type};
use std::{
    io,
    ops::Range,
    path::{Path, PathBuf},
};

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

    pub fn place_at(&mut self, pos: usize) {
        self.base = pos;
        self.pos = pos;
        // TODO: Reset desired position
    }

    pub fn select(&mut self, range: Range<usize>) {
        self.base = range.start;
        self.pos = range.end;
        // TODO: Reset desired position
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
    // TODO: Remove this
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

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
    pub unsaved: bool,
    pub text: Text,
    pub highlights: Option<Highlights>,
    pub cursors: HopSlotMap<CursorId, Cursor>,
    pub dir: Option<PathBuf>,
    pub path: Option<PathBuf>,
}

impl Buffer {
    pub fn from_file(path: PathBuf) -> Result<Self, Error> {
        let (unsaved, dir, chars, s) = match std::fs::read_to_string(&path) {
            Ok(s) => {
                let mut path = path.canonicalize()?;
                path.pop();
                (false, Some(path), s.chars().collect(), s)
            }
            // If the file doesn't exist, create a new file
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let dir = path
                    .parent()
                    .filter(|p| p.to_str() != Some(""))
                    .map(Path::to_owned)
                    .or_else(|| std::env::current_dir().ok());
                (true, dir, Vec::new(), String::new())
            }
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            unsaved,
            highlights: Highlighter::from_file_name(&path).map(|h| h.highlight(&chars)),
            text: Text { chars },
            cursors: HopSlotMap::default(),
            dir,
            path: Some(path),
        })
    }

    pub fn save(&mut self) -> Result<(), Error> {
        if self.unsaved {
            std::fs::write(
                self.path.as_ref().expect("buffer must have path to save"),
                self.text.to_string(),
            )?;
            self.unsaved = false;
        }
        Ok(())
    }

    pub fn name(&self) -> Option<String> {
        Some(
            match self.path.as_ref()?.file_name().and_then(|n| n.to_str()) {
                Some(name) => format!("{}{name}", if self.unsaved { "* " } else { "" }),
                None => "<error>".to_string(),
            },
        )
    }

    fn update_highlights(&mut self) {
        self.highlights = self
            .highlights
            .take()
            .map(|hl| hl.highlighter.highlight(self.text.chars()));
    }

    pub fn clear(&mut self) {
        self.unsaved = true;

        self.text.chars.clear();
        self.update_highlights();
        // Reset cursors
        self.cursors.values_mut().for_each(|cursor| {
            *cursor = Cursor::default();
        });
    }

    pub fn goto_line_cursor(&mut self, cursor_id: CursorId, line: isize) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        cursor.pos = self.text.to_pos([0, line]);
        cursor.reset_desired_col(&self.text);
        cursor.base = cursor.pos;
    }

    pub fn select_token_cursor(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        if let Some(tok) = self
            .highlights
            .as_ref()
            // Choose the longest token that the cursor is touching
            .and_then(|hl| {
                let a = hl.get_at(cursor.pos);
                let b = hl.get_at(cursor.pos.saturating_sub(1));
                a.zip(b)
                    .map(|(a, b)| {
                        if a.range.end - a.range.start > b.range.end - b.range.start {
                            a
                        } else {
                            b
                        }
                    })
                    .or(a)
                    .or(b)
            })
        {
            cursor.select(tok.range.clone());
        } else {
            // TODO: Bell
        }
    }

    fn indent_at(&mut self, pos: usize) {
        const TAB_ALIGN: usize = 4;

        let coord = self.text.to_coord(pos).map(|e| e.max(0) as usize);
        let next_up = |x: usize, n: usize| (x / n + 1) * n;
        let n = next_up(coord[0], TAB_ALIGN) - coord[0];
        self.insert(pos, (0..n).map(|_| ' '));
    }

    pub fn indent(&mut self, cursor_id: CursorId, forward: bool) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        if let Some(range) = cursor.selection() {
        } else {
            let pos = cursor.pos;
            self.indent_at(pos);
        }
    }

    pub fn move_cursor(
        &mut self,
        cursor_id: CursorId,
        dir: Dir,
        dist: [usize; 2],
        retain_base: bool,
        word: bool,
    ) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        match dir {
            Dir::Left => {
                cursor.pos = if !retain_base && cursor.base < cursor.pos {
                    cursor.base
                } else if let (true, Some(mut pos)) = (word, cursor.pos.checked_sub(1)) {
                    let class = self.text.chars().get(pos).copied().map(classify);
                    loop {
                        pos = match pos.checked_sub(1) {
                            Some(pos) if self.text.chars().get(pos).copied().map(classify) == class => pos,
                            _ => break pos,
                        }
                    }
                } else {
                    cursor.pos.saturating_sub(dist[0])
                };
                cursor.reset_desired_col(&self.text);
            }
            Dir::Right => {
                cursor.pos = if !retain_base && cursor.base > cursor.pos {
                    cursor.base
                } else if word {
                    let mut pos = cursor.pos;
                    let class = self.text.chars().get(pos).copied().map(classify);
                    loop {
                        pos = if self.text.chars().get(pos).copied().map(classify) == class {
                            pos + 1
                        } else {
                            break pos
                        };
                    }
                } else {
                    (cursor.pos + dist[0]).min(self.text.chars.len())
                };
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

    pub fn insert(&mut self, pos: usize, chars: impl IntoIterator<Item = char>) {
        self.unsaved = true;

        let mut n = 0;
        for c in chars {
            self.text
                .chars
                .insert((pos + n).min(self.text.chars.len()), c);
            n += 1;
        }
        self.update_highlights();
        self.cursors.values_mut().for_each(|cursor| {
            if cursor.base >= pos {
                cursor.base += n;
            }
            if cursor.pos >= pos {
                cursor.pos += n;
                cursor.reset_desired_col(&self.text);
            }
        });
    }

    pub fn enter(&mut self, cursor_id: CursorId, chars: impl IntoIterator<Item = char>) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
            self.enter(cursor_id, chars);
        } else {
            self.insert(cursor.pos, chars);
        }
    }

    // Assumes range is well-formed
    pub fn remove(&mut self, range: Range<usize>) {
        self.unsaved = true;

        // TODO: Bell if false?
        self.text.chars.drain(range.clone());
        self.update_highlights();
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

// CLassify the character by property
fn classify(c: char) -> u8 {
    match c {
        // c if c.is_ascii_whitespace() => 0,
        c if c.is_alphanumeric() || c == '_' => 1,
        _ => 2,
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
    pub fn open_or_get(&mut self, path: PathBuf) -> Result<BufferId, Error> {
        let true_path = path.canonicalize()?;
        if let Some((buffer_id, _)) = self.buffers.iter().find(|(_, b)| {
            b.path.as_ref().and_then(|p| p.canonicalize().ok()).as_ref() == Some(&true_path)
        }) {
            Ok(buffer_id)
        } else {
            Ok(self.buffers.insert(Buffer::from_file(path)?))
        }
    }

    pub fn tick(&mut self) {
        self.tick += 1;
    }
}
