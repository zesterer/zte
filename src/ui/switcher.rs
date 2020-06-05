use vek::*;
use crate::{
    Canvas,
    Event,
    Dir,
};
use super::{
    Context,
    Element,
};

pub struct Switcher {
    selected_idx: usize,
}

impl Switcher {
    pub fn new(ctx: &mut Context) -> Self {
        Self { selected_idx: 0 }
    }
}

impl Element for Switcher {
    type Response = ();

    fn handle(&mut self, ctx: &mut Context, event: Event) {
        let recent_count = ctx.state.recent_buffers().len();
        match event {
            Event::CursorMove(Dir::Up) => self.selected_idx = (self.selected_idx + recent_count.saturating_sub(1)) % recent_count,
            Event::CursorMove(Dir::Down) => self.selected_idx = (self.selected_idx + 1) % recent_count,
            Event::Insert('\n') => {
                ctx.secondary_events.push_back(Event::CloseMenu);
                ctx.secondary_events.push_back(Event::SwitchBuffer({
                    let old_handle = ctx.state
                        .recent_buffers()
                        .nth(self.selected_idx)
                        .unwrap();
                    ctx.state.clone_handle(old_handle).unwrap()
                }));
            },
            _ => {},
        }
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        // Todo
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let mut canvas = canvas.window(Rect::new(
            sz.w / 4,
            sz.h / 4,
            sz.w - sz.w / 2,
            sz.h - sz.h / 2,
        ));

        // Frame
        let sz = canvas.size();
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
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

        let title = format!("[Recent Buffers]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        // Entries
        let mut canvas = canvas.window(Rect::new(
            1,
            1,
            canvas.size().w - 2,
            canvas.size().h - 2,
        ));

        let handles = ctx.state.recent_buffers().collect::<Vec<_>>();
        for (i, handle) in handles.iter().enumerate().take(canvas.size().h) {
            if i == self.selected_idx {
                canvas.write_char(Vec2::new(0, i), '>');
            }

            let buf = ctx.state.get_shared_buffer(handle.buffer_id).unwrap();
            canvas.write_str(Vec2::new(2, i), buf.title());
        }
    }
}
