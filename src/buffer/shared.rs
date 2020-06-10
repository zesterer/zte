use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
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
    CharKind,
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

#[derive(Clone)]
struct State {
    pub content: Content,
    cursors: HashMap<CursorId, Cursor>,
}

impl State {
    // Make this state align with another according to undo/redo rules (i.e: preserving cursor
    // positions where possible)
    fn align_with(&mut self, other: Self) {
        self.content = other.content;
        for (id, c) in other.cursors.into_iter() {
            self.cursors.insert(id, c);
        }
    }
}

const MAX_UNDO_STATES: usize = 256;

pub struct SharedBuffer {
    state: State,
    past_states: VecDeque<State>,
    future_states: Vec<State>,
    config: Config,
    pub path: Option<PathBuf>,
    cursor_id_counter: usize,
    unsaved: bool,
}

impl SharedBuffer {
    fn pre_edit(&mut self) {
        self.past_states.push_front(self.state.clone());
        while self.past_states.len() > MAX_UNDO_STATES {
            self.past_states.pop_back();
        }
        self.future_states.clear();
    }

    fn undo(&mut self) {
        if let Some(s) = self.past_states.pop_front() {
            self.future_states.push(self.state.clone());
            self.state.align_with(s);
        }
    }

    fn redo(&mut self) {
        if let Some(s) = self.future_states.pop() {
            self.past_states.push_front(self.state.clone());
            self.state.align_with(s);
        }
    }

    fn new_id(&mut self) -> usize {
        self.cursor_id_counter += 1;
        self.cursor_id_counter
    }

    pub fn cursor(&self, id: CursorId) -> &Cursor {
        self.state.cursors.get(&id).unwrap()
    }

    pub fn cursor_mut(&mut self, id: CursorId) -> &mut Cursor {
        self.state.cursors.get_mut(&id).unwrap()
    }

    fn is_unsaved(&self) -> bool {
        self.unsaved
    }

    fn trigger_mutation(&mut self) {
        self.unsaved = true;
    }

    fn remove_cursor(&mut self, id: &CursorId) {
        self.state.cursors.remove(id);
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
        self.insert_at(self.cursor(id).pos, c);
    }

    pub fn insert_at(&mut self, pos: usize, c: char) {
        self.state.content.insert(pos, c);
        self.state.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, 1));
        self.trigger_mutation();
    }

    fn insert_line(&mut self, line: usize, s: &str) {
        self.state.content.insert_line(line, s);
    }

    fn backspace(&mut self, id: CursorId) {
        let pos = self.cursor(id).pos;
        if pos > 0 {
            self.remove_at(pos - 1);
        }
    }

    fn delete(&mut self, id: CursorId) {
        self.remove_at(self.cursor(id).pos);
    }

    fn remove_at(&mut self, pos: usize) {
        self.state.content.remove(pos);
        self.state.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, -1));
        self.trigger_mutation();
    }

    pub fn content(&self) -> &Content {
        &self.state.content
    }

    pub fn insert_cursor(&mut self, cursor: Cursor) -> CursorId {
        let id = self.new_id();
        self.state.cursors.insert(CursorId(id), cursor);
        CursorId(id)
    }

    pub fn try_save(&mut self) -> Result<(), io::Error> {
        if let Some(path) = &self.path {
            let mut f = File::create(path)?;
            for c in self.content().chars() {
                f.write(c.encode_utf8(&mut [0; 4]).as_bytes())?;
            }
            self.unsaved = false;
            self.path = Some(path.canonicalize()?);
        }

        Ok(())
    }

    pub fn open_or_create(path: PathBuf) -> Result<Self, SharedBufferError> {
        let (content, unsaved) = if let Ok(mut file) = File::open(&path) {
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            (Content::from(buf), false)
        } else {
            (Content::default(), true)
        };

        Ok(Self {
            path: Some(path.canonicalize().unwrap_or(path)),
            state: State {
                content,
                cursors: HashMap::new(),
            },
            unsaved,
            ..Self::default()
        })
    }
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self {
            config: Config::default(),
            state: State {
                content: Content::default(),
                cursors: HashMap::new(),
            },
            past_states: VecDeque::new(),
            future_states: Vec::new(),
            path: None,
            cursor_id_counter: 0,
            unsaved: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BufferHandle {
    pub buffer_id: BufferId,
    pub cursor_id: CursorId,
    pub rc: Arc<()>,
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
        self.buffer.content()
    }

    pub fn title(&self) -> &str {
        self.buffer.title()
    }

    pub fn is_unsaved(&self) -> bool {
        self.buffer.is_unsaved()
    }

    pub fn len(&self) -> usize {
        self.buffer.content().len()
    }

    pub fn line_count(&self) -> usize {
        self.buffer.content().lines().len()
    }

    pub fn line(&self, line: usize) -> Option<Line> {
        self.buffer.content().line(line)
    }
	
	pub fn current_line(&self) -> Line {
		self.line(self.cursor_loc().y).unwrap()
	}

    pub fn cursor(&self) -> &Cursor {
        self.buffer.cursor(self.cursor_id)
    }

    pub fn cursor_mut(&mut self) -> &mut Cursor {
        self.buffer.cursor_mut(self.cursor_id)
    }

    pub fn undo(&mut self) {
        self.buffer.undo();
    }

    pub fn redo(&mut self) {
        self.buffer.redo();
    }

    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor_id, c);
    }

    pub fn insert_at(&mut self, pos: usize, c: char) {
        self.buffer.insert_at(pos, c);
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

    pub fn selection(&self) -> impl Iterator<Item=char> + ExactSizeIterator<Item=char> + '_ {
        let from = self.cursor().base.min(self.cursor().pos);
        let to = self.cursor().base.max(self.cursor().pos);
        (from..to)
            .map(move |i| self.content().char_at(i).unwrap())
    }

	pub fn cursor_loc(&self) -> Vec2<usize> {
		self.pos_loc(self.cursor().pos, self.config())
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

    pub fn cursor_move(&mut self, dir: Dir, n: usize) -> bool {
        let old_pos = self.cursor().pos;
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
        self.cursor().pos != old_pos
    }

    fn get_next_char(&self, dir: Dir) -> Option<char> {
        match dir {
            Dir::Left => self.content().char_at(self.cursor().pos.saturating_sub(1)),
            Dir::Right => self.content().char_at(self.cursor().pos),
            _ => unimplemented!(),
        }
    }

    pub fn cursor_jump(&mut self, dir: Dir) {
        //self.cursor_move(dir, 1);

        if let Dir::Left | Dir::Right = dir {
            // Consume any whitespace before
            while matches!(self.get_next_char(dir).map(CharKind::from_char), Some(None)) {
                let old_pos = self.cursor().pos;
                self.cursor_move(dir, 1);
                if self.cursor().pos == old_pos {
                    break;
                }
            }

            let kind = match self.get_next_char(dir) {
                Some(c) => CharKind::from_char(c),
                None => return,
            };

            while self.get_next_char(dir).map(CharKind::from_char) == Some(kind) {
                let old_pos = self.cursor().pos;
                self.cursor_move(dir, 1);
                if self.cursor().pos == old_pos {
                    break;
                }
            }
        }
    }

    pub fn backspace_word(&mut self) {
        // Consume any whitespace before
        while self.get_next_char(Dir::Left).map(|c| c != '\n' && c.is_whitespace()).unwrap_or(false)
            && self.cursor().pos > 0
        {
            self.backspace();
        }

        let kind = match self.get_next_char(Dir::Left) {
            Some(c) => CharKind::from_char(c),
            None => return,
        };

        while self.get_next_char(Dir::Left).map(CharKind::from_char) == Some(kind)
            && self.cursor().pos > 0
        {
            self.backspace();
        }
    }

    fn remove_selection(&mut self) {
        let len = self.selection().len();
        for _ in 0..len {
            self.buffer.remove_at(self.cursor().base.min(self.cursor().pos));
        }
        self.cursor_mut().pos = self.cursor().base.min(self.cursor().pos);
    }

    pub fn do_cursor_movement(&mut self, dir: Dir, reach: bool, f: impl FnOnce(&mut Self)) {
        let can_do = if !reach {
            !self.cursor_mut().unreach(dir)
        } else {
            true
        };
        if can_do {
            f(self)
        }
        if !reach {
            self.cursor_mut().reset_base();
        }
    }

    pub fn handle(&mut self, event: Event) {
        match event {
            // Do not mutate
            Event::CursorMove(dir, reach) => return self.do_cursor_movement(dir, reach, |b| { b.cursor_move(dir, 1); }),
            Event::CursorJump(dir, reach) => return self.do_cursor_movement(dir, reach, |b| { b.cursor_jump(dir); }),
            _ => {},
        }

        self.buffer.pre_edit();

        match event {
            // Mutate
			Event::Insert(c) => {
                self.remove_selection();
                match c {
                    '\n' if self.config().auto_indent => {
		        		let whitespace = self
        					.current_line()
				        	.chars()
		        			.take_while(|c| c.is_whitespace() && *c != '\n')
        					.collect::<Vec<_>>();
				        self.insert('\n');
				        for ws in whitespace {
		        			self.insert(ws);
        				}
			        },
                    '\t' if !self.config().hard_tabs => {
				        self.insert(' ');
				        while self.cursor_loc().x % self.config().tab_width != 0 {
					        self.insert(' ');
				        }
			        },
                    c => self.insert(c),
                }
            },
            Event::Backspace if self.cursor().is_reaching() => self.remove_selection(),
            Event::Backspace if !self.config().hard_tabs => {
                self.backspace();
                let base_x = self.cursor_loc().x - self.cursor_loc().x % self.config().tab_width;
                if (base_x..self.cursor_loc().x)
                    .all(|i| self.current_line().get(i).map(|c| c == ' ').unwrap_or(false))
                    && (self.cursor_loc().x + 1) % self.config().tab_width == 0
                {
                    for _ in base_x..self.cursor_loc().x {
                        self.backspace();
                    }
                }
            },
            Event::Backspace => self.backspace(),
            Event::BackspaceWord => self.backspace_word(),
            Event::Delete => self.delete(),
            Event::Duplicate if self.cursor().is_reaching() => self.insert_str(&self.selection().collect::<String>()),
            Event::Duplicate => self.duplicate_line(),
            Event::Cut => {
                let _ = ClipboardContext::new()
                    .and_then(|mut ctx| ctx.set_contents(self.selection().collect()));
                self.remove_selection();
            },
            Event::Copy => {
                let _ = ClipboardContext::new()
                    .and_then(|mut ctx| ctx.set_contents(self.selection().collect()));
            },
            Event::Paste => match ClipboardContext::new().and_then(|mut ctx| ctx.get_contents()) {
                Ok(s) => self.insert_str(&s),
                Err(_) => {},
            },
            _ => {},
        }
    }
}
