use vek::*;
use crate::{
    Canvas,
    Event,
    Dir,
    buffer::shared::{SharedBuffer, BufferGuard},
    CursorId,
    Cursor,
};
use super::{
    Context,
    Element,
};

pub struct Prompt {
    buffer: SharedBuffer,
    cursor_id: CursorId,
}

impl Default for Prompt {
    fn default() -> Self {
        let mut buffer = SharedBuffer::default();
        let cursor_id = buffer.insert_cursor(Cursor::default());

        Self {
            buffer,
            cursor_id,
        }
    }
}

impl Prompt {
    pub fn buf_mut(&mut self) -> BufferGuard {
        BufferGuard {
            buffer: &mut self.buffer,
            cursor_id: self.cursor_id,
        }
    }

    pub fn get_text(&self) -> String {
        let line = self
            .buffer
            .content()
            .line(0)
            .unwrap();
        line.chars().take(line.len() - 1).collect::<String>()
    }

    pub fn append(&mut self, s: &str) {
        let line_len = self
            .buffer
            .content()
            .line(0)
            .unwrap()
            .len() - 1;

        for (i, c) in s.chars().enumerate() {
            self.buffer.insert_at(line_len + i, c);
        }

        self.buf_mut().cursor_set(Vec2::new(line_len + s.len(), 0));
    }
}

impl Element for Prompt {
    type Response = ();

    fn handle(&mut self, ctx: &mut Context, event: Event) {
        match event {
            Event::Insert('\n') => {},
            event => self.buf_mut().handle(event),
        }
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        // Todo
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
        for (i, c) in self.get_text().chars().enumerate().take(canvas.size().w) {
            canvas.write_char(Vec2::new(i, 0), c);
        }

        if active {
            let cursor_pos = Vec2::new(self.buffer.cursor(self.cursor_id).pos, 0);
            canvas.set_cursor(Some(cursor_pos));
        }
    }
}
