use std::{
    collections::HashMap,
    rc::Rc,
    cell::{RefCell, Ref, RefMut},
};
use super::{
    Line,
    Config,
    Cursor,
    Buffer,
    BufferMut,
    Content,
};

#[derive(Hash, PartialEq, Eq)]
pub struct CursorId(usize);

pub struct SharedBuffer {
    config: Config,
    content: Content,
    cursor_id_counter: usize,
    cursors: HashMap<CursorId, Cursor>,
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

    fn remove_cursor(&mut self, id: &CursorId) {
        self.cursors.remove(id);
    }

    fn insert(&mut self, id: &CursorId, c: char) {
        let pos = self.cursor(id).pos;
        self.content.insert(pos, c);
        self.cursors
            .values_mut()
            .filter(|cursor| cursor.pos >= pos)
            .for_each(|cursor| cursor.pos += 1);
    }

    fn backspace(&mut self, id: &CursorId) {
        let pos = self.cursor(id).pos;
        if pos > 0 {
            self.content.remove(pos - 1);
            self.cursors
                .values_mut()
                .filter(|cursor| cursor.pos >= pos)
                .for_each(|cursor| cursor.pos -= 1);
        }
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
            cursor_id_counter: 0,
            cursors: HashMap::new(),
        }
    }
}

pub struct SharedBufferRef {
    buffer: Rc<RefCell<SharedBuffer>>,
    cursor_id: CursorId,
}

impl SharedBufferRef {
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

impl Default for SharedBufferRef {
    fn default() -> Self {
        SharedBuffer::make_ref(Rc::new(RefCell::new(SharedBuffer::default())))
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

impl<'a> SharedBufferGuard<'a> {
    pub fn config(&self) -> &Config {
        &self.buffer.config
    }
}

impl<'a> Buffer for SharedBufferGuard<'a> {
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

impl<'a> SharedBufferGuardMut<'a> {
    pub fn config(&self) -> &Config {
        &self.buffer.config
    }
}

impl<'a> Buffer for SharedBufferGuardMut<'a> {
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
}
