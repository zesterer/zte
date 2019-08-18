use std::{
    collections::HashMap,
    rc::Rc,
    cell::{RefCell, Ref, RefMut},
    path::PathBuf,
    fs::File,
    io::{self, Read, Write},
};
use super::{
    Line,
    Config,
    Cursor,
    Buffer,
    BufferMut,
    Content,
};

#[derive(Debug)]
pub enum SharedBufferError {
    Io(io::Error),
}

impl From<io::Error> for SharedBufferError {
    fn from(err: io::Error) -> Self {
        SharedBufferError::Io(err)
    }
}

#[derive(Hash, PartialEq, Eq)]
pub struct CursorId(usize);

pub struct SharedBuffer {
    config: Config,
    path: Option<PathBuf>,
    content: Content,
    cursor_id_counter: usize,
    cursors: HashMap<CursorId, Cursor>,
    unsaved: bool,
}

impl SharedBuffer {
    fn new_id(&mut self) -> usize {
        self.cursor_id_counter += 1;
        self.cursor_id_counter
    }

    fn insert_cursor(&mut self, cursor: Cursor) -> CursorId {
        let id = self.new_id();
        self.cursors.insert(CursorId(id), cursor);
        CursorId(id)
    }

    fn cursor(&self, id: &CursorId) -> &Cursor {
        self.cursors.get(id).unwrap()
    }

    fn cursor_mut(&mut self, id: &CursorId) -> &mut Cursor {
        self.cursors.get_mut(id).unwrap()
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

    fn title(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|path| path
                .file_name()
                .and_then(|s| s
                    .to_str()))
            .unwrap_or("untitled")
    }

    fn insert(&mut self, id: &CursorId, c: char) {
        let pos = self.cursor(id).pos;
        self.content.insert(pos, c);
        self.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, 1));
        self.trigger_mutation();
    }

    fn backspace(&mut self, id: &CursorId) {
        let pos = self.cursor(id).pos;
        if pos > 0 {
            self.content.remove(pos - 1);
            self.cursors
                .values_mut()
                .for_each(|cursor| cursor.shift_relative_to(pos - 1, -1));
            self.trigger_mutation();
        }
    }

    fn delete(&mut self, id: &CursorId) {
        let pos = self.cursor(id).pos;
        self.content.remove(pos);
        self.cursors
            .values_mut()
            .for_each(|cursor| cursor.shift_relative_to(pos, -1));
        self.trigger_mutation();
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

    pub fn make_ref(this: Rc<RefCell<Self>>) -> SharedBufferRef {
        SharedBufferRef {
            buffer: this.clone(),
            cursor_id: this.borrow_mut().insert_cursor(Cursor::default())
        }
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

pub struct SharedBufferRef {
    buffer: Rc<RefCell<SharedBuffer>>,
    cursor_id: CursorId,
}

impl SharedBufferRef {
    pub fn new(s: String, path: Option<PathBuf>) -> Self {
        Self::from(SharedBuffer {
            content: Content::from(s),
            path,
            unsaved: false,
            ..Default::default()
        })
    }

    pub fn open(path: PathBuf) -> Result<Self, SharedBufferError> {
        let mut buf = String::new();
        File::open(&path)?.read_to_string(&mut buf)?;
        Ok(Self::new(buf, Some(path)))
    }

    pub fn in_use(&self) -> bool {
        Rc::strong_count(&self.buffer) == 1
    }

    pub fn borrow(&self) -> SharedBufferGuard {
        SharedBufferGuard {
            buffer: self.buffer.borrow(),
            cursor_id: &self.cursor_id,
        }
    }

    pub fn borrow_mut(&mut self) -> SharedBufferGuardMut {
        SharedBufferGuardMut {
            buffer: self.buffer.borrow_mut(),
            cursor_id: &self.cursor_id,
        }
    }
}

impl From<SharedBuffer> for SharedBufferRef {
    fn from(buf: SharedBuffer) -> Self {
        SharedBuffer::make_ref(Rc::new(RefCell::new(buf)))
    }
}

impl Default for SharedBufferRef {
    fn default() -> Self {
        Self::from(SharedBuffer::default())
    }
}

impl Clone for SharedBufferRef {
    fn clone(&self) -> Self {
        SharedBuffer::make_ref(self.buffer.clone())
    }
}

impl Drop for SharedBufferRef {
    fn drop(&mut self) {
        self.buffer
            .borrow_mut()
            .remove_cursor(&self.cursor_id);
    }
}

// SharedBufferGuard

pub struct SharedBufferGuard<'a> {
    buffer: Ref<'a, SharedBuffer>,
    cursor_id: &'a CursorId,
}

impl<'a> Buffer for SharedBufferGuard<'a> {
    type Error = SharedBufferError;

    fn config(&self) -> &Config {
        &self.buffer.config
    }

    fn title(&self) -> &str {
        self.buffer.title()
    }

    fn is_unsaved(&self) -> bool {
        self.buffer.is_unsaved()
    }

    fn len(&self) -> usize {
        self.buffer.content.len()
    }

    fn line_count(&self) -> usize {
        self.buffer.content.lines().len()
    }

    fn line(&self, line: usize) -> Option<Line> {
        self.buffer.content.line(line)
    }

    fn cursor(&self) -> &Cursor {
        self.buffer.cursor(self.cursor_id)
    }
}

// SharedBufferGuardMut

pub struct SharedBufferGuardMut<'a> {
    buffer: RefMut<'a, SharedBuffer>,
    cursor_id: &'a CursorId,
}

impl<'a> Buffer for SharedBufferGuardMut<'a> {
    type Error = SharedBufferError;

    fn config(&self) -> &Config {
        &self.buffer.config
    }

    fn title(&self) -> &str {
        self.buffer.title()
    }

    fn is_unsaved(&self) -> bool {
        self.buffer.is_unsaved()
    }

    fn len(&self) -> usize {
        self.buffer.content.len()
    }

    fn line_count(&self) -> usize {
        self.buffer.content.lines().len()
    }

    fn line(&self, line: usize) -> Option<Line> {
        self.buffer.content.line(line)
    }

    fn cursor(&self) -> &Cursor {
        self.buffer.cursor(self.cursor_id)
    }
}

impl<'a> BufferMut for SharedBufferGuardMut<'a> {
    fn cursor_mut(&mut self) -> &mut Cursor {
        self.buffer.cursor_mut(self.cursor_id)
    }

    fn insert(&mut self, c: char) {
        self.buffer.insert(&self.cursor_id, c);
    }

    fn backspace(&mut self) {
        self.buffer.backspace(&self.cursor_id);
    }

    fn delete(&mut self) {
        self.buffer.delete(&self.cursor_id);
    }

    fn try_save(&mut self) -> Result<(), io::Error> {
        self.buffer.try_save()
    }
}
