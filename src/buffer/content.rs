use vek::*;
use super::Line;

pub struct Content {
    lines: Vec<Vec<char>>,
}

impl Content {
    pub fn len(&self) -> usize {
        self
            .lines()
            .map(|line| line.len())
            .sum::<usize>().saturating_sub(1)
    }

    pub fn lines(&self) -> impl ExactSizeIterator<Item=Line> {
        self.lines
            .iter()
            .map(|line| Line::from(line.as_slice()))
    }

    pub fn line(&self, line: usize) -> Option<Line> {
        self
            .lines()
            .skip(line)
            .next()
    }

    pub fn chars(&self) -> impl Iterator<Item=char> + '_ {
        self.lines
            .iter()
            .map(|line| line
                .iter()
                .copied()
                .chain(Some('\n')))
            .flatten()
    }

    pub fn pos_to_rank_line(&self, mut pos: usize) -> Vec2<usize> {
        let mut row = 0;
        for line in self.lines() {
            if pos >= line.len() {
                row += 1;
                pos -= line.len();
            } else {
                break;
            }
        }
        Vec2::new(pos, row)
    }

    pub fn insert(&mut self, pos: usize, c: char) {
        let (rank, line) = self.pos_to_rank_line(pos).into_tuple();

        if self.lines.len() <= line {
            self.lines.push(Vec::new());
        }

        match c {
            '\n' => {
                let tail = self.lines[line].split_off(rank);
                self.lines.insert(line + 1, tail);
            },
            c => self.lines[line].insert(rank, c),
        }
    }

    pub fn insert_line(&mut self, line: usize, s: &str) {
        self.lines.insert(line, s.chars().collect());
    }

    pub fn remove(&mut self, pos: usize) {
        let (rank, line) = self.pos_to_rank_line(pos).into_tuple();

        if self.lines.len() <= line {
            self.lines.push(Vec::new());
        }

        if self.lines
            .get(line)
            .map(|l| rank == l.len())
            .unwrap_or(false)
        {
            if line < self.lines.len() - 1 {
                let mut old_line = self.lines.remove(line + 1);
                self.lines[line].append(&mut old_line);
            }
        } else if self.lines.get(line).is_some() {
            self.lines[line].remove(rank);
        }
    }
}

impl<T: AsRef<str>> From<T> for Content {
    fn from(s: T) -> Self {
        Self {
            lines: s
                .as_ref()
                .lines()
                .map(|l| l.chars().collect())
                .collect(),
        }
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::from("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_line() {
        let content = Content::from("hello");

        assert_eq!(content.lines().len(), 1);
        assert_eq!(content.lines().next().unwrap().len(), 6);
        assert_eq!(content.pos_loc(0), Vec2::new(0, 0));
        assert_eq!(content.pos_loc(3), Vec2::new(3, 0));
        assert_eq!(content.pos_loc(4), Vec2::new(4, 0));
        assert_eq!(content.pos_loc(5), Vec2::new(5, 0));

        let content = Content::from("hello\nworld\ntest");

        assert_eq!(content.lines().len(), 3);
        assert_eq!(content.lines().skip(2).next().unwrap().len(), 5);
        assert_eq!(content.pos_loc(5), Vec2::new(5, 0));
        assert_eq!(content.pos_loc(6), Vec2::new(0, 1));
        assert_eq!(content.pos_loc(11), Vec2::new(5, 1));
    }
}
