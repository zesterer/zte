use vek::*;
use crate::{
    draw::*,
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

const MARGIN_WIDTH: usize = 5;
const CURSOR_SPACE: Vec2<usize> = Vec2 { x: 4, y: 4 };
const PAGE_LENGTH: usize = 24;

pub struct Editor {
    loc: Vec2<usize>,
    buffer: SharedBufferRef,
}

impl Default for Editor {
    fn default() -> Self {
        Self::from(SharedBufferRef::default())
    }
}

impl From<SharedBufferRef> for Editor {
    fn from(buffer: SharedBufferRef) -> Self {
        Self {
            loc: Vec2::zero(),
            buffer,
        }
    }
}

impl Element for Editor {
    type Response = ();

    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::Insert(c) => self.buffer.borrow_mut().insert(c),
            Event::Backspace => self.buffer.borrow_mut().backspace(),
            Event::Delete => self.buffer.borrow_mut().delete(),
            Event::CursorMove(dir) => self.buffer.borrow_mut().cursor_move(dir, 1),
            Event::PageMove(dir) => self.buffer.borrow_mut().cursor_move(dir, PAGE_LENGTH),
            Event::SaveBuffer => self.buffer.borrow_mut().try_save().unwrap(),
            _ => {},
        }
    }

    fn update(&mut self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        let canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));

        let buf = self.buffer.borrow();

        let cursor_loc = buf.pos_loc(buf.cursor().pos, buf.config());

        self.loc.x = self.loc.x
            .min(cursor_loc.x.saturating_sub(CURSOR_SPACE.x))
            .max(cursor_loc.x.saturating_sub(canvas.size().w - MARGIN_WIDTH - CURSOR_SPACE.x));
        self.loc.y = self.loc.y
            .min(cursor_loc.y.saturating_sub(CURSOR_SPACE.y))
            .max(cursor_loc.y.saturating_sub(canvas.size().h - CURSOR_SPACE.y));
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
        let title = format!("[{}{}]", if buf.is_unsaved() { "*" } else { "" }, buf.title());
        for (i, c) in title.chars()
            .enumerate()
        {
            canvas.set(Vec2::new((sz.w.saturating_sub(title.len())) / 2 + i, 0), c.into());
        }

        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));

        for row in 0..canvas.size().h {
            let buf_row = row + self.loc.y;

            let (line, margin) = match buf.line(buf_row) {
                Some(line) => (line, format!("{:>4} ", buf_row)),
                None => (Line::empty(), "     ".to_string()),
            };

            // Margin
            for (col, c) in margin
                .chars()
                .enumerate()
            {
                canvas
                    .with_fg(ctx.theme.line_number_color)
                    .with_bg(ctx.theme.margin_color)
                    .write_char(Vec2::new(col, row), c);
            }

            // Text
            for (col, (_, c)) in line
                .glyphs(&buf.config())
                .skip(self.loc.x)
                .enumerate()
                .take(canvas.size().w.saturating_sub(MARGIN_WIDTH))
            {
                let buf_col = col + self.loc.x;

                canvas
                    .write_char(Vec2::new(MARGIN_WIDTH + col, row), c);
            }
        }

        if active {
            let cursor_loc = buf.pos_loc(buf.cursor().pos, &buf.config());
            let cursor_screen_loc = cursor_loc.map2(self.loc, |e, loc| e.saturating_sub(loc)) + Vec2::unit_x() * MARGIN_WIDTH;
            canvas.set_cursor(Some(cursor_screen_loc).filter(|loc| loc.x < canvas.size().w && loc.y < canvas.size().h));
        }
    }
}
