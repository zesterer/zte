use vek::*;
use crate::{
    draw::*,
    BufferId,
    BufferHandle,
    Line,
    Event,
    Dir,
    buffer::{highlight::Highlights, shared::SharedBuffer},
};
use super::{
    Context,
    Element,
};

const MARGIN_WIDTH: usize = 5;
const CURSOR_SPACE: Vec2<usize> = Vec2 { x: 4, y: 4 };
const PAGE_HEIGHT: usize = 24;

pub struct Editor {
    loc: Vec2<usize>,
    buffer: BufferHandle,
    page_height: usize,
}

impl From<BufferHandle> for Editor {
    fn from(buffer: BufferHandle) -> Self {
        Self {
            loc: Vec2::zero(),
            buffer,
            page_height: PAGE_HEIGHT,
        }
    }
}

impl Editor {
    pub fn empty(ctx: &mut Context) -> Self {
        let buffer = ctx.state.new_empty_buffer();
        Self::from(ctx.state.new_handle(buffer).unwrap())
    }

    pub fn recent(ctx: &mut Context) -> Self {
        let recent_buf = ctx.state
            .recent_buffers()
            .next()
            .cloned();
        recent_buf
            .map(|b| Self::from(ctx.state.duplicate_handle(&b).unwrap()))
            .unwrap_or_else(|| Self::empty(ctx))
    }

    pub fn buffer(&self) -> &BufferHandle {
        &self.buffer
    }
}

impl Element for Editor {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
        let mut buf = ctx.state
            .get_buffer_mut(&self.buffer)
            .unwrap();

        match event {
            Event::CloseBuffer { force } => if !force && ctx.state
                .get_buffer(&self.buffer)
                .map_or(false, |b| b.is_unsaved())
            {
                ctx.secondary_events.push_back(Event::ConfirmCloseBuffer(self.buffer.clone()));
            } else {
                let buf = ctx.state
                    .recent_buffers()
                    .find(|buf| buf.buffer_id != self.buffer.buffer_id)
                    .cloned();
                if let Some(buf) = buf {
                    let old_buffer = self.buffer.buffer_id;
                    self.buffer = buf;
                    ctx.state.close_buffer(old_buffer);
                }
            },
            Event::NewBuffer => {
                let buf = ctx.state.insert_buffer(SharedBuffer::default());
                self.buffer = ctx.state.new_handle(buf).unwrap();
            },
            Event::SaveBuffer => buf.try_save().unwrap(),
            Event::SwitchBuffer(buffer) => self.buffer = buffer,
            Event::PageMove(dir, reach) => buf.do_cursor_movement(dir, reach, |b| { b.cursor_move(dir, self.page_height); }),
            Event::Undo => buf.undo(),
            Event::Redo => buf.redo(),
            Event::NewFile(path) | Event::OpenFile(path) => match ctx
                .state
                .open_or_create_file(path, self.buffer.clone())
            {
                Ok(buf) => self.buffer = buf,
                Err(err) => log::warn!("When opening file: {:?}", err),
            },
            event => buf.handle(event)?,
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));

        self.page_height = canvas.size().h;

        // Update the most recent buffer with this one
        if active {
            ctx.state.set_recent_buffer(self.buffer.clone());
            ctx.active_buffer = self.buffer.buffer_id;
        }

        let buf = ctx.state
            .get_buffer_mut(&self.buffer)
            .unwrap();

        let cursor_loc = buf.pos_loc(buf.cursor().pos);

        self.loc.x = self.loc.x
            .min(cursor_loc.x.saturating_sub(CURSOR_SPACE.x))
            .max(cursor_loc.x.saturating_sub(canvas.size().w.saturating_sub(MARGIN_WIDTH + CURSOR_SPACE.x)));
        self.loc.y = self.loc.y
            .min(cursor_loc.y.saturating_sub(CURSOR_SPACE.y))
            .max(cursor_loc.y.saturating_sub(canvas.size().h.saturating_sub(CURSOR_SPACE.y)));
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let buf = ctx.state
            .get_buffer_mut(&self.buffer)
            .unwrap();

        // Frame
        canvas
            .with_fg(Color::Rgb(if active {
                Rgb::broadcast(255)
            } else {
                Rgb::broadcast(100)
            }))
            .frame();

        // Title
        let title = format!("[ {}{} ]", if buf.is_unsaved() { "*" } else { "" }, buf.title());
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));

        let highlights = Highlights::from_file(buf.path(), &buf.get_string());

        let cursor_loc = buf.pos_loc(buf.cursor().pos);

        for row in 0..canvas.size().h {
            let buf_row = row + self.loc.y;
            let buf_row_pos = buf.loc_pos(Vec2::new(0, buf_row));

            let (line, margin) = match buf.line(buf_row) {
                Some(line) => (line, format!("{:>4} ", buf_row + 1)),
                None => (Line::empty(), "     ".to_string()),
            };

            // Margin
            for (col, c) in margin
                .chars()
                .enumerate()
            {
                let line_selected = buf.cursor().encloses(buf_row_pos) && row < buf.content().lines().len();
                canvas
                    .with_fg(if line_selected {
                        Color::Rgb(Rgb::broadcast(255))
                    } else {
                        ctx.theme.line_number_color
                    })
                    .with_bg(if line_selected {
                        ctx.theme.selection_color
                    } else {
                        ctx.theme.margin_color
                    })
                    .write_char(Vec2::new(col, row), c);
            }

            // Text
            for (col, (line_pos, c)) in line
                .glyphs(&buf.config())
                .skip(self.loc.x)
                .enumerate()
                .take(canvas.size().w.saturating_sub(MARGIN_WIDTH))
            {
                let buf_col = col + self.loc.x;
                let buf_pos = buf_row_pos + line_pos.unwrap_or(line.len().saturating_sub(1));

                let bg_color = if buf.cursor().inside_reach(buf_pos) /*&& line_pos.is_some()*/ {
                    ctx.theme.selection_color
                } else if buf_row == cursor_loc.y {
                    ctx.theme.subtle_bg_color
                } else {
                    Color::Reset
                };

                canvas
                    .with_fg(ctx.theme.get_highlight_color(highlights.get_at(buf_pos)))
                    .with_bg(bg_color)
                    .write_char(Vec2::new(MARGIN_WIDTH + col, row), c);
            }
        }

        if active {
            let cursor_screen_loc = cursor_loc.map2(self.loc, |e, loc| e.saturating_sub(loc)) + Vec2::unit_x() * MARGIN_WIDTH;
            canvas.set_cursor(Some(cursor_screen_loc).filter(|loc| loc.x < canvas.size().w && loc.y < canvas.size().h));
        }

        // Scrollbar
        let pad_h = (sz.h.saturating_sub(2).pow(2) / buf.content().lines().len().max(1)).max(1);
        if pad_h < sz.h.saturating_sub(2) {
            // Bg
            canvas
                .with_fg(ctx.theme.scrollbar_color)
                .with_bg(ctx.theme.margin_color)
                .rectangle((sz.w - 2, 0), (1, sz.h - 2), '│');

            // Pad
            canvas
                .with_fg(ctx.theme.scrollbar_color)
                .with_bg(ctx.theme.scrollpad_color)
                .rectangle((sz.w - 2, ((self.loc.y * sz.h.saturating_sub(1)) as f32 / buf.content().lines().len().max(1) as f32) as usize), (1, pad_h), '.');
        }
    }
}
