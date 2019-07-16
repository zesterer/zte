mod shared;
mod content;

// Reexports
pub use self::{
    shared::SharedBufferRef,
    content::Content,
};

use vek::*;

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
            .chain((0..).map(|_| (None, ' ')))
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

pub trait Buffer {
    fn len(&self) -> usize;
    fn line_count(&self) -> usize;
    fn line(&self, line: usize) -> Option<Line>;

    fn cursor(&self) -> &Cursor;

    fn lines(&self) -> Box<dyn Iterator<Item=Line> + '_> {
        Box::new((0..self.line_count())
            .scan(0, move |_, l| self.line(l)))
    }

    fn pos_loc(&self, mut pos: usize, cfg: &Config) -> Vec2<usize> {
        let mut row = 0;
        for line in self.lines() {
            if pos >= line.len() {
                row += 1;
                pos -= line.len();
            } else {
                break;
            }
        }

        let mut col = 0;
        match self.line(row) {
            Some(line) => for (p, _) in line.glyphs(cfg) {
                match p {
                    Some(p) if p == pos => break,
                    Some(_) => col += 1,
                    None => break,
                }
            },
            None => {},
        }
        Vec2::new(col, row)
    }

    fn loc_pos(&self, loc: Vec2<usize>, cfg: &Config) -> usize {
        let mut pos = self
            .lines()
            .take(loc.y)
            .map(|l| l.len())
            .sum();

        pos += match self.line(loc.y) {
            Some(line) => line
                .glyphs(cfg)
                .take(loc.x)
                .next()
                .unwrap()
                .0
                .unwrap_or(line.len() - 1),
            None => 0,
        };

        pos
    }
}

pub trait BufferMut {
    fn cursor_mut(&mut self) -> &mut Cursor;

    fn insert(&mut self, c: char);
    fn backspace(&mut self);
}
