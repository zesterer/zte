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

enum Action {
    Quit,
    CloseBuffer(BufferHandle),
}

pub struct Confirm {
    action: Action,
}

impl Confirm {
    pub fn quit(ctx: &mut Context) -> Self {
        Self { action: Action::Quit }
    }
    
    pub fn close_buffer(buffer: BufferHandle) -> Self {
        Self { action: Action::CloseBuffer(buffer) }
    }

    pub fn cancel(self, ctx: &mut Context) {}
}

impl Element for Confirm {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
        let recent_count = ctx.state.recent_buffers().len();
        match (&self.action, event) {
            (Action::Quit, Event::Insert('y')) => return Err(Event::Quit),
            (Action::CloseBuffer(_), Event::Insert('y')) => {
                ctx.secondary_events.push_back(Event::CloseMenu);
                return Err(Event::CloseBuffer { force: true })
            },
            (_, Event::Insert('n')) => ctx.secondary_events.push_back(Event::CloseMenu),
            (_, event @ Event::Escape) => return Err(event),
            (_, _) => {},
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        // Todo
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let mut canvas = canvas.window(Rect::new(
            (sz.w / 2).saturating_sub(sz.w / 3),
            (sz.h / 2).saturating_sub(sz.h / 6),
            sz.w * 2 / 3,
            sz.h * 2 / 6,
        ));

        // Frame
        let sz = canvas.size();
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
        canvas.frame();

        let title = format!("[Confirm]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        // Entries
        let mut canvas = canvas.window(Rect::new(
            1,
            1,
            canvas.size().w.saturating_sub(2),
            canvas.size().h.saturating_sub(2),
        ));

        let text = match &self.action {
            Action::Quit => format!(
                "Quit and lose unsaved work? (y/n)\n\nThe following files are unsaved:\n{}",
                ctx.state
                    .recent_buffers()
                    .flat_map(|b| ctx.state.get_buffer(b))
                    .filter(|b| b.is_unsaved())
                    .map(|b| b.title())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            Action::CloseBuffer(buffer) => format!(
                "Close `{}` and lose unsaved changes? (y/n)",
                ctx.state.get_buffer(&buffer).map_or("", |b| b.title()),
            ),
        };
        for (i, line) in text.lines().enumerate() {
            let x = canvas.size().w.saturating_sub(line.chars().count()) / 2;
            canvas.write_str(Vec2::new(x, i + 1), line);
        }
    }
}
