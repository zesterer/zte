pub mod shared;
pub mod content;
pub mod highlight;

// Reexports
pub use self::{
    shared::{BufferId, BufferHandle},
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

    pub fn chars(&self) -> impl Iterator<Item=char> + '_ {
        self.chars
            .iter()
            .copied()
            .chain(Some('\n'))
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
    pub pos: usize,
    pub reach: isize,
}

impl Cursor {
    pub fn shift_relative_to(&mut self, pos: usize, dist: isize) {
        if self.pos >= pos {
            self.pos = (self.pos as isize + dist).max(pos as isize) as usize;
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            pos: 0,
            reach: 0,
        }
    }
}

pub struct Config {
    tab_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
        }
    }
}

// pub trait Buffer {
//     type Error: Debug;

//     fn config(&self) -> &Config;

//     fn title(&self) -> &str;
//     fn is_unsaved(&self) -> bool;

//     fn len(&self) -> usize;
//     fn line_count(&self) -> usize;
//     fn line(&self, line: usize) -> Option<Line>;

//     fn cursor(&self) -> &Cursor;

//     fn lines(&self) -> Box<dyn Iterator<Item=Line> + '_> {
//         Box::new((0..self.line_count())
//             .scan(0, move |_, l| self.line(l)))
//     }

//     fn get_string(&self) -> String {
//         let mut s = String::new();
//         self.lines().for_each(|line| s.extend(line.chars()));
//         s
//     }

//     fn pos_loc(&self, mut pos: usize, cfg: &Config) -> Vec2<usize> {
//         let mut row = 0;
//         for line in self.lines() {
//             if pos >= line.len() {
//                 row += 1;
//                 pos -= line.len();
//             } else {
//                 break;
//             }
//         }

//         let mut col = 0;
//         match self.line(row) {
//             Some(line) => for (p, _) in line.glyphs(cfg) {
//                 match p {
//                     Some(p) if p == pos => break,
//                     Some(_) => col += 1,
//                     None => break,
//                 }
//             },
//             None => {},
//         }
//         Vec2::new(col, row)
//     }

//     fn loc_pos(&self, loc: Vec2<usize>, cfg: &Config) -> usize {
//         let mut pos = (0..loc.y)
//             .map(|l| self.line(l).map(|l| l.len()).unwrap_or(0))
//             .sum::<usize>();

//         pos += match self.line(loc.y) {
//             Some(line) => line
//                 .glyphs(cfg)
//                 .skip(loc.x)
//                 .next()
//                 .unwrap()
//                 .0
//                 .unwrap_or(line.len() - 1),
//             None => 0,
//         };

//         pos.min(self.len())
//     }
// }

// pub trait BufferMut: Buffer {
//     fn cursor_mut(&mut self) -> &mut Cursor;

//     fn insert(&mut self, c: char);
//     fn backspace(&mut self);
//     fn delete(&mut self);
//     fn insert_line(&mut self, line: usize, s: &str);

//     fn try_save(&mut self) -> Result<(), io::Error>;

//     fn duplicate_line(&mut self) {
//         let row = self.pos_loc(self.cursor().pos, self.config()).y;
//         if let Some(line) = self.line(row) {
//             let s = line.get_string();
//             self.insert_line(row + 1, &s);
//         }
//     }

//     fn insert_str(&mut self, s: &str) {
//         for c in s.chars() {
//             self.insert(c);
//         }
//     }

//     fn cursor_move(&mut self, dir: Dir, n: usize) {
//         match dir {
//             Dir::Left => self.cursor_mut().pos = self.cursor().pos.saturating_sub(n),
//             Dir::Right => self.cursor_mut().pos = (self.cursor().pos + n).min(self.len()),
//             Dir::Up => {
//                 let cursor_loc = self.pos_loc(self.cursor().pos, self.config());
//                 if cursor_loc.y == 0 {
//                     self.cursor_mut().pos = 0;
//                 } else {
//                     self.cursor_mut().pos = self.loc_pos(Vec2::new(cursor_loc.x, cursor_loc.y.saturating_sub(n)), self.config());
//                 }
//             },
//             Dir::Down => {
//                 let cursor_loc = self.pos_loc(self.cursor().pos, self.config());
//                 if cursor_loc.y == self.line_count() {
//                     self.cursor_mut().pos = self.len() + 1;
//                 } else {
//                     self.cursor_mut().pos = self.loc_pos(Vec2::new(cursor_loc.x, cursor_loc.y + n), self.config());
//                 }
//             },
//         }
//     }
// }
