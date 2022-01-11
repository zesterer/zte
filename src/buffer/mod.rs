pub mod shared;
pub mod content;
pub mod highlight;

// Reexports
pub use self::{
    shared::{BufferId, BufferHandle, CursorId},
    content::Content,
};

use std::{
    fmt::Debug,
    io,
};
use vek::*;
use crate::Dir;

pub struct Line<'a> {
    chars: &'a [char],
}

impl<'a> Line<'a> {
    pub fn empty() -> Self {
        Self {
            chars: &[],
        }
    }

    pub fn len(&self) -> usize {
        self.chars.len() + 1
    }

    pub fn get(&self, pos: usize) -> Option<char> {
        self.chars.get(pos).copied()
    }

    pub fn chars(&self) -> impl Iterator<Item=char> + '_ {
        self.chars
            .iter()
            .copied()
            .chain(std::iter::once('\n'))
    }

    pub fn glyphs(&self, cfg: &Config) -> impl Iterator<Item=(Option<usize>, char)> + '_ {
        let tab_width = cfg.tab_width;
        self
            .chars()
            .enumerate()
            .scan(0, move |col, (pos, c)| Some(match c {
                '\t' => {
                    let padding = (*col / tab_width + 1) * tab_width - *col;
                    *col += padding;
                    (padding, (pos, ' '))
                },
                '\n' => (0, (pos, '\n')),
                c => {
                    *col += 1;
                    (1, (pos, c))
                },
            }))
            .map(|(n, (pos, c))| (0..n).map(move |_| (Some(pos), c)))
            .flatten()
            .chain(std::iter::repeat((None, ' ')))
    }

    pub fn get_string(&self) -> String {
        self.chars().collect()
    }
}

impl<'a> From<&'a [char]> for Line<'a> {
    fn from(chars: &'a [char]) -> Self {
        Self { chars }
    }
}

#[derive(Copy, Clone)]
pub struct Cursor {
    pub base: usize,
    pub pos: usize,
}

impl Cursor {
    pub fn unreach(&mut self, dir: Dir) -> bool {
        if self.base == self.pos {
            false
        } else {
            self.pos = if dir.is_forward() {
                self.base.max(self.pos)
            } else {
                self.base.min(self.pos)
            };
            true
        }
    }

    pub fn reset_base(&mut self) {
        self.base = self.pos;
    }

    pub fn is_reaching(&self) -> bool {
        self.base != self.pos
    }

    pub fn shift_relative_to(&mut self, pos: usize, dist: isize) {
        if self.pos >= pos {
            self.pos = (self.pos as isize + dist).max(pos as isize) as usize;
        }
        if self.base >= pos {
            self.base = (self.base as isize + dist).max(pos as isize) as usize;
        }
    }

    pub fn inside_reach(&self, pos: usize) -> bool {
        pos >= self.base && pos < self.pos || pos >= self.pos && pos < self.base
    }

    pub fn encloses(&self, pos: usize) -> bool {
        pos > self.base && pos <= self.pos || pos > self.pos && pos <= self.base
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            base: 0,
            pos: 0,
        }
    }
}

pub struct Config {
    tab_width: usize,
    hard_tabs: bool,
	auto_indent: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            hard_tabs: false,
			auto_indent: true,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum CharKind {
    AlphaNum,
    Other,
}

impl CharKind {
    pub fn from_char(c: char) -> Option<Self> {
        if c != '\n' && c.is_whitespace() {
            None
        } else if c.is_alphanumeric() || c == '_' {
            Some(CharKind::AlphaNum)
        } else {
            Some(CharKind::Other)
        }
    }
}
