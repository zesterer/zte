use vek::*;
use crate::{
    Canvas,
    SharedBufferRef,
    Buffer,
    BufferMut,
    Line,
    Event,
    Dir,
};
use super::{
    Context,
    Element,
};

pub struct Editor {
    buffer: SharedBufferRef,
}

impl Default for Editor {
    fn default() -> Self {
        Self::from(SharedBufferRef::default())
    }
}

impl From<SharedBufferRef> for Editor {
    fn from(buffer: SharedBufferRef) -> Self {
        Self { buffer }
    }
}

impl Element for Editor {
    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::Insert(c) => self.buffer.borrow_mut().insert(c),
            Event::Backspace => self.buffer.borrow_mut().backspace(),
            Event::Delete => self.buffer.borrow_mut().delete(),
            Event::CursorMove(dir) => self.buffer.borrow_mut().cursor_move(dir, 1),
            _ => {},
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let buf = self.buffer.borrow();

        // Frame
        for i in 1..sz.w - 1 {
            canvas.set(Vec2::new(i, 0), '-'.into());
            canvas.set(Vec2::new(i, sz.h - 1), '-'.into());
        }
        for j in 1..sz.h - 1 {
            canvas.set(Vec2::new(0, j), '|'.into());
            canvas.set(Vec2::new(sz.w - 1, j), '|'.into());
        }
        canvas.set(Vec2::new(0, 0), '.'.into());
        canvas.set(Vec2::new(sz.w - 1, 0), '.'.into());
        canvas.set(Vec2::new(0, sz.h - 1), '\''.into());
        canvas.set(Vec2::new(sz.w - 1, sz.h - 1), '\''.into());

        // Title
        let title = format!("[{}]", buf.title());
        for (i, c) in title.chars()
            .enumerate()
        {
            canvas.set(Vec2::new((sz.w.saturating_sub(title.len())) / 2 + i, 0), c.into());
        }

        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));

        const MARGIN_WIDTH: usize = 5;

        for row in 0..canvas.size().h {
            let (line, margin) = match buf.line(row) {
                Some(line) => (line, format!("{:>4} ", row)),
                None => (Line::empty(), "     ".to_string()),
            };

            // Margin
            for (col, c) in margin
                .chars()
                .enumerate()
            {
                canvas.set(Vec2::new(col, row), c.into());
            }

            // Text
            for (col, (_, c)) in line
                .glyphs(&buf.config())
                .enumerate()
                .take(canvas.size().w.saturating_sub(MARGIN_WIDTH))
            {
                canvas.set(Vec2::new(MARGIN_WIDTH + col, row), c.into());
            }
        }

        if active {
            let loc = buf.pos_loc(buf.cursor().pos, &buf.config()) + Vec2::unit_x() * MARGIN_WIDTH;
            canvas.set_cursor(Some(loc).filter(|loc| loc.x < canvas.size().w && loc.y < canvas.size().h));
        }
    }
}
