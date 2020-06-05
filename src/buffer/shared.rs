use std::{
    collections::HashMap,
    rc::Rc,
    cell::{RefCell, Ref, RefMut},
    path::PathBuf,
    fs::File,
    io::{self, Read, Write},
    cmp::PartialEq,
};
use clipboard::{ClipboardContext, ClipboardProvider};
use vek::*;
use crate::{Dir, Event};
use super::{
    Line,
    Config,
    Cursor,
    Content,
};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct BufferId(pub usize);

#[derive(Debug)]
pub enum SharedBufferError {
    Io(io::Error),
}

impl From<io::Error> for SharedBufferError {
    fn from(err: io::Error) ->  Self {
        SharedBufferError::Io(err)
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct CursorId(usize);

pub struct SharedBuffer {
    config: Config,
    pub path: Option<PathBuf>,
    pub content: Content,
    cursor_id_counter: usize,
    cursors: HashMap<CursorId, Cursor>,
    unsaved: bool,
}

impl SharedBuffer {
    fn new_id(&mut self) -> usize {
        self.cursor_id_counter += 1;
        self.cursor_id_counter
    }

    pub fn cursor(&self, id: CursorId) -> &Cursor {
        self.cursors.get(&id).unwrap()
    }

    pub fn cursor_mut(&mut self, id: CursorId) -> &mut Cursor {
        self.cursors.get_mut(&id).unwrap()
    }

    fn is_unsaved(&self) -> bool {
        self.unsaved
    }

    fn trigger_mutation(&mut self) {
        self.unsaved = true;
    }

    fn remove_cursor(&mut self, id: &CursorId) {
        self.cursors.remove(id);
    }

    pub fn title(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|path| path
                .file_name()
                .and_then(|s| s
                    .to_str()))
            .unwrap_or("untitled")
    }

    fn insert(&mut self, id: CursorId, c: char) {
        let pos = self.cursor(id).pos;
        self.content.insert(pos, c);
        self.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, 1));
        self.trigger_mutation();
    }

    fn insert_line(&mut self, line: usize, s: &str) {
        self.content.insert_line(line, s);
    }

    fn backspace(&mut self, id: CursorId) {
        let pos = self.cursor(id).pos;
        if pos > 0 {
            self.content.remove(pos - 1);
            self.cursors
                .values_mut()
                .for_each(|cursor| cursor.shift_relative_to(pos - 1, -1));
            self.trigger_mutation();
        }
    }

    fn delete(&mut self, id: CursorId) {
        let pos = self.cursor(id).pos;
        self.content.remove(pos);
        self.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, -1));
        self.trigger_mutation();
    }

    pub fn insert_cursor(&mut self, cursor: Cursor) -> CursorId {
        let id = self.new_id();
        self.cursors.insert(CursorId(id), cursor);
        CursorId(id)
    }

    pub fn try_save(&mut self) -> Result<(), io::Error> {
        if let Some(path) = &self.path {
            let mut f = File::create(path)?;
            for c in self.content.chars() {
                f.write(c.encode_utf8(&mut [0; 4]).as_bytes())?;
            }
            self.unsaved = false;
        }

        Ok(())
    }

    pub fn open(path: PathBuf) -> Result<Self, SharedBufferError> {
        let content = if let Ok(mut file) = File::open(&path) {
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            Content::from(buf)
        } else {
            Content::default()
        };

        Ok(Self {
            path: Some(path.canonicalize().unwrap()),
            content,
            unsaved: false,
            ..Self::default()
        })
    }
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self {
            config: Config::default(),
            content: Content::default(),
            path: None,
            cursor_id_counter: 0,
            cursors: HashMap::new(),
            unsaved: true,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BufferHandle {
    pub buffer_id: BufferId,
    pub cursor_id: CursorId,
}

pub struct BufferGuard<'a> {
    pub buffer: &'a mut SharedBuffer,
    pub cursor_id: CursorId,
}

impl<'a> BufferGuard<'a> {
    pub fn config(&self) -> &Config {
        &self.buffer.config
    }
    pub fn content(&self) -> &Content {
        &self.buffer.content
    }

    pub fn title(&self) -> &str {
        self.buffer.title()
    }

    pub fn is_unsaved(&self) -> bool {
        self.buffer.is_unsaved()
    }

    pub fn len(&self) -> usize {
        self.buffer.content.len()
    }

    pub fn line_count(&self) -> usize {
        self.buffer.content.lines().len()
    }

    pub fn line(&self, line: usize) -> Option<Line> {
        self.buffer.content.line(line)
    }

    pub fn cursor(&self) -> &Cursor {
        self.buffer.cursor(self.cursor_id)
    }

    pub fn cursor_mut(&mut self) -> &mut Cursor {
        self.buffer.cursor_mut(self.cursor_id)
    }

    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor_id, c);
    }

    pub fn insert_line(&mut self, line: usize, s: &str) {
        self.buffer.insert_line(line, s);
    }

    pub fn backspace(&mut self) {
        self.buffer.backspace(self.cursor_id);
    }

    pub fn delete(&mut self) {
        self.buffer.delete(self.cursor_id);
    }

    pub fn try_save(&mut self) -> Result<(), io::Error> {
        self.buffer.try_save()
    }

    pub fn lines(&self) -> Box<dyn Iterator<Item=Line> + '_> {
        Box::new((0..self.line_count())
            .scan(0, move |_, l| self.line(l)))
    }

    pub fn get_string(&self) -> String {
        let mut s = String::new();
        self.lines().for_each(|line| s.extend(line.chars()));
        s
    }

    pub fn pos_loc(&self, mut pos: usize, cfg: &Config) -> Vec2<usize> {
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

    pub fn loc_pos(&self, loc: Vec2<usize>, cfg: &Config) -> usize {
        let mut pos = (0..loc.y)
            .map(|l| self.line(l).map(|l| l.len()).unwrap_or(0))
            .sum::<usize>();

        pos += match self.line(loc.y) {
            Some(line) => line
                .glyphs(cfg)
                .skip(loc.x)
                .next()
                .unwrap()
                .0
                .unwrap_or(line.len() - 1),
            None => 0,
        };

        pos.min(self.len())
    }

    pub fn duplicate_line(&mut self) {
        let row = self.pos_loc(self.cursor().pos, self.config()).y;
        if let Some(line) = self.line(row) {
            let s = line.get_string();
            self.insert_line(row + 1, &s);
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert(c);
        }
    }

    pub fn cursor_set(&mut self, loc: Vec2<usize>) {
        self.cursor_mut().pos = self.loc_pos(loc, self.config());
    }

    pub fn cursor_move(&mut self, dir: Dir, n: usize) {
        match dir {
            Dir::Left => self.cursor_mut().pos = self.cursor().pos.saturating_sub(n),
            Dir::Right => self.cursor_mut().pos = (self.cursor().pos + n).min(self.len()),
            Dir::Up => {
                let cursor_loc = self.pos_loc(self.cursor().pos, self.config());
                if cursor_loc.y == 0 {
                    self.cursor_mut().pos = 0;
                } else {
                    self.cursor_mut().pos = self.loc_pos(Vec2::new(cursor_loc.x, cursor_loc.y.saturating_sub(n)), self.config());
                }
            },
            Dir::Down => {
                let cursor_loc = self.pos_loc(self.cursor().pos, self.config());
                if cursor_loc.y == self.line_count() {
                    self.cursor_mut().pos = self.len() + 1;
                } else {
                    self.cursor_set(Vec2::new(cursor_loc.x, cursor_loc.y + n));
                }
            },
        }
    }

    pub fn handle(&mut self, event: Event) {
        match event {
            Event::Insert(c) => self.insert(c),
            Event::Backspace => self.backspace(),
            Event::Delete => self.delete(),
            Event::CursorMove(dir) => self.cursor_move(dir, 1),
            Event::Paste => match ClipboardContext::new().and_then(|mut ctx| ctx.get_contents()) {
                Ok(s) => self.insert_str(&s),
                Err(_) => {},
            },
            _ => {},
        }
    }
}
