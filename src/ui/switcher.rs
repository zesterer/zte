use vek::*;
use crate::{
    Canvas,
    Event,
    Dir,
    BufferHandle,
};
use super::{
    Context,
    Element,
};

pub struct Switcher {
    selected_idx: usize,
    prev_buffer: BufferHandle,
}

impl Switcher {
    pub fn new(ctx: &mut Context, prev_buffer: BufferHandle) -> Self {
        Self { selected_idx: 0, prev_buffer }
    }

    pub fn cancel(self, ctx: &mut Context) {
        ctx.secondary_events.push_back(Event::SwitchBuffer(self.prev_buffer));
    }
}

impl Element for Switcher {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
        let recent_count = ctx.state.recent_buffers().len();
        match event {
            Event::CursorMove(dir, _) => {
                match dir {
                    Dir::Up => self.selected_idx = (self.selected_idx + recent_count.saturating_sub(1)) % recent_count,
                    Dir::Down => self.selected_idx = (self.selected_idx + 1) % recent_count,
                    _ => return Err(event),
                }
                ctx.secondary_events.push_back(Event::SwitchBuffer({
                    let old_handle = ctx.state
                        .recent_buffers()
                        .nth(self.selected_idx)
                        .unwrap()
                        .clone();
                    ctx.state.duplicate_handle(&old_handle).unwrap()
                }));
            },
            Event::Insert('\n') => ctx.secondary_events.push_back(Event::CloseMenu),
            _ => return Err(event),
        }
        Ok(())
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
        canvas.frame();

        let title = format!("[Recent Buffers]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        // Entries
        let mut canvas = canvas.window(Rect::new(
            1,
            1,
            canvas.size().w - 2,
            canvas.size().h - 2,
        ));

        let handles = ctx.state.recent_buffers().cloned().collect::<Vec<_>>();
        for (i, handle) in handles.iter().enumerate().take(canvas.size().h) {
            if i == self.selected_idx {
                canvas.write_char(Vec2::new(0, i), '>');
            }

            let buf = ctx.state.get_shared_buffer(handle.buffer_id).unwrap();
            canvas.write_str(Vec2::new(2, i), buf.title());
        }
    }
}
