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
            Event::CursorMove(dir) => self.buffer.borrow_mut().cursor_move(dir),
            _ => {},
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        canvas
            .with_bg(ctx.theme.frame_bg_color)
            .frame((0, 0), sz);
        canvas
            .with_bg(ctx.theme.editor_bg_color)
            .rectangle((1, 1), sz.map(|e| e - 2), ' ');

        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w - 2, canvas.size().h - 2));

        for row in 0..canvas.size().h {
            let buf = self.buffer.borrow();
            for (col, (_, c)) in buf.line(row).unwrap_or(Line::empty()).glyphs(&buf.config())
                .enumerate()
                .take(canvas.size().w)
            {
                canvas.set(Vec2::new(col, row), c.into());
            }
        }

        if active {
            let buf = self.buffer.borrow();
            let loc = buf.pos_loc(buf.cursor().pos, &buf.config());
            canvas.set_cursor(Some(loc).filter(|loc| loc.x < canvas.size().w && loc.y < canvas.size().h));
        }
    }
}
