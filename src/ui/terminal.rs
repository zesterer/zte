use vek::*;
use crate::{
    draw::*,
    Event,
    Dir,
};
use super::{
    Context,
    Element,
};

#[derive(Default)]
pub struct Terminal;

impl Element for Terminal {
    type Response = ();

    fn handle(&mut self, ctx: &mut Context, event: Event) {
        // Todo
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        // Todo
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();

        // Frame
        let mut frame_canvas = canvas.with_fg(Color::Rgb(if active {
            Rgb::broadcast(255)
        } else {
            Rgb::broadcast(100)
        }));
        for i in 1..sz.w.saturating_sub(1) {
            frame_canvas.write_char(Vec2::new(i, 0), '-');
            frame_canvas.write_char(Vec2::new(i, sz.h.saturating_sub(1)), '-');
        }
        for j in 1..sz.h.saturating_sub(1) {
            frame_canvas.write_char(Vec2::new(0, j), '|'.into());
            frame_canvas.write_char(Vec2::new(sz.w.saturating_sub(1), j), '|');
        }
        frame_canvas.write_char(Vec2::new(0, 0), '.'.into());
        frame_canvas.write_char(Vec2::new(sz.w.saturating_sub(1), 0), '.');
        frame_canvas.write_char(Vec2::new(0, sz.h.saturating_sub(1)), '\'');
        frame_canvas.write_char(Vec2::new(sz.w.saturating_sub(1), sz.h.saturating_sub(1)), '\'');

        // Title
        let title = format!("[Terminal]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w.saturating_sub(2), canvas.size().h.saturating_sub(2)));
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
    }
}
