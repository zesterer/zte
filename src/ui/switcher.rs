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

pub struct Switcher;

impl Default for Switcher {
    fn default() -> Self {
        Self
    }
}

impl Element for Switcher {
    type Response = ();

    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::CursorMove(Dir::Up) => {},
            Event::CursorMove(Dir::Down) => {},
            _ => {},
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
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
    }
}
