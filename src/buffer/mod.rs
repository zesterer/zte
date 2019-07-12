use std::collections::HashMap;
use vek::*;

pub struct Line {
    chars: Vec<char>,
}

impl Line {
    pub fn len(&self) -> usize {
        self.chars.len() + 1
    }
}

impl<'a> From<&'a str> for Line {
    fn from(s: &'a str) -> Self {
        Self {
            chars: s.chars().filter(|c| *c != '\n').collect(),
        }
    }
}

pub struct Buffer {
    lines: Vec<Line>,
    cursor_id_counter: usize,
    cursors: HashMap<usize, Cursor>,
}

impl Buffer {
    fn pos_loc(&self, mut pos: usize) -> Vec2<usize> {
        for (i, line) in self.lines().enumerate() {
            if pos >= line.len() {
                pos -= line.len();
            } else {
                return Vec2::new(pos, i);
            }
        }
        Vec2::new(0, self.lines().len())
    }

    pub fn lines(&self) -> impl ExactSizeIterator<Item=&Line> {
        self.lines.iter()
    }
}

impl<'a> From<&'a str> for Buffer {
    fn from(s: &'a str) -> Self {
        Self {
            lines: s
                .lines()
                .map(|l| Line::from(l))
                .collect(),
            cursor_id_counter: 0,
            cursors: HashMap::new(),
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            cursor_id_counter: 0,
            cursors: HashMap::new(),
        }
    }
}

pub struct Cursor(usize);

impl Default for Cursor {
    fn default() -> Self {
        Self(0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_line() {
        let buf = Buffer::from("hello");

        assert_eq!(buf.lines().len(), 1);
        assert_eq!(buf.lines().next().unwrap().len(), 6);
        assert_eq!(buf.pos_loc(0), Vec2::new(0, 0));
        assert_eq!(buf.pos_loc(3), Vec2::new(3, 0));
        assert_eq!(buf.pos_loc(4), Vec2::new(4, 0));
        assert_eq!(buf.pos_loc(5), Vec2::new(5, 0));

        let buf = Buffer::from("hello\nworld\ntest");

        assert_eq!(buf.lines().len(), 3);
        assert_eq!(buf.lines().skip(2).next().unwrap().len(), 5);
        assert_eq!(buf.pos_loc(5), Vec2::new(5, 0));
        assert_eq!(buf.pos_loc(6), Vec2::new(0, 1));
        assert_eq!(buf.pos_loc(11), Vec2::new(5, 1));
    }
}
