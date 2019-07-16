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
        Self {
            buffer: SharedBufferRef::default(),
        }
    }
}

impl Element for Editor {
    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::Insert(c) => self.buffer.borrow_mut().insert(c),
            Event::Backspace => self.buffer.borrow_mut().backspace(),
            Event::CursorMove(dir) => {
                let mut buf = self.buffer.borrow_mut();
                match dir {
                    Dir::Left => buf.cursor_mut().pos = buf.cursor_mut().pos.saturating_sub(1),
                    Dir::Right => buf.cursor_mut().pos = (buf.cursor().pos + 1).min(buf.len()),
                    _ => unimplemented!(),
                }
            },
            _ => {},
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        canvas
            .with_bg(ctx.theme.editor_bg_color)
            .rectangle((0, 0), (-1, -1), ' ');
        canvas
            .with_bg(ctx.theme.frame_bg_color)
            .frame((0, 0), (-1, -1));

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
            canvas.set_cursor(Some(loc));
        }
    }
}
