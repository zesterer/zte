// Reexports
pub use crate::display::{
    Color,
    Attr,
};

use vek::*;
use crate::display::{
    Cell,
    Display,
};

fn rect_to_points(canvas: &impl Canvas, pos: Vec2<usize>, size: Extent2<isize>) -> (Vec2<usize>, Vec2<usize>) {
    let this_size = canvas.size();
    let size = size.map2((Vec2::from(this_size) - pos).into(), |e, sz: usize| if e < 0 {
        sz as isize + e + 1
    } else {
        e
    } as usize);

    (pos, pos + size)
}

pub trait Canvas: Sized {
    fn set_cursor_raw(&mut self, pos: Option<Vec2<usize>>);
    fn set_raw(&mut self, pos: Vec2<usize>, cell: Cell);
    fn rect(&self) -> Rect<usize, usize>;
    fn size(&self) -> Extent2<usize>;

    fn fg(&self) -> Color;
    fn bg(&self) -> Color;
    fn attr(&self) -> Attr;

    fn set_cursor(&mut self, pos: Option<Vec2<usize>>) {
        self.set_cursor_raw(pos.map(|pos| self.rect().position() + pos));
    }

    fn set(&mut self, pos: Vec2<usize>, cell: Cell) {
        let pos = pos.map2(self.size().into(), |e, sz: usize| e.min(sz.saturating_sub(1)));
        self.set_raw(self.rect().position() + pos, cell);
    }

    fn write_char(&mut self, pos: Vec2<usize>, c: char) {
        self.set(pos, Cell(c, self.fg(), self.bg(), self.attr()));
    }

    fn write_str(&mut self, pos: Vec2<usize>, s: &str) {
        for (i, c) in s.chars().enumerate() {
            self.write_char(pos + Vec2::new(i, 0), c);
        }
    }

    fn with_fg<'a>(&'a mut self, fg: Color) -> Drawer<'a, Self> {
        Drawer { fg, bg: self.bg(), attr: self.attr(), rect: self.rect(), canvas: self }
    }

    fn with_bg<'a>(&'a mut self, bg: Color) -> Drawer<'a, Self> {
        Drawer { fg: self.fg(), bg, attr: self.attr(), rect: self.rect(), canvas: self }
    }

    fn with_attr<'a>(&'a mut self, attr: Attr) -> Drawer<'a, Self> {
        Drawer { fg: self.fg(), bg: self.bg(), attr, rect: self.rect(), canvas: self }
    }

    fn window<'a>(&'a mut self, rect: Rect<usize, usize>) -> Drawer<'a, Self> {
        let rect = Rect::new(
            self.rect().position().x + rect.x,
            self.rect().position().y + rect.y,
            rect.w.min(self.rect().w.saturating_sub(rect.x)),
            rect.h.min(self.rect().h.saturating_sub(rect.y)),
        );
        Drawer { fg: self.fg(), bg: self.bg(), attr: self.attr(), rect, canvas: self }
    }

    fn rectangle(&mut self, pos: impl Into<Vec2<usize>>, size: impl Into<Extent2<usize>>, c: char) {
        let from = pos.into();
        let to = from + Vec2::from(size.into());

        let cell = Cell(c, self.fg(), self.bg(), self.attr());
        for y in from.y..to.y {
            for x in from.x..to.x {
                self.set(Vec2::new(x, y), cell);
            }
        }
    }

    fn frame(&mut self) {
        let sz = self.size();
        for i in 1..sz.w.saturating_sub(1) {
            self.write_char(Vec2::new(i, 0), '─');
            self.write_char(Vec2::new(i, sz.h.saturating_sub(1)), '─');
        }
        for j in 1..sz.h.saturating_sub(1) {
            self.write_char(Vec2::new(0, j), '│'.into());
            self.write_char(Vec2::new(sz.w.saturating_sub(1), j), '│');
        }
        self.write_char(Vec2::new(0, 0), '┌'.into());
        self.write_char(Vec2::new(sz.w.saturating_sub(1), 0), '┐');
        self.write_char(Vec2::new(0, sz.h.saturating_sub(1)), '└');
        self.write_char(Vec2::new(sz.w.saturating_sub(1), sz.h.saturating_sub(1)), '┘');
    }
}

pub struct Drawer<'a, D: Canvas> {
    fg: Color,
    bg: Color,
    attr: Attr,
    canvas: &'a mut D,
    rect: Rect<usize, usize>,
}

impl<'a, D: Canvas> Canvas for Drawer<'a, D> {
    fn set_cursor_raw(&mut self, pos: Option<Vec2<usize>>) {
        self.canvas.set_cursor_raw(pos);
    }

    fn set_raw(&mut self, pos: Vec2<usize>, cell: Cell) {
        self.canvas.set_raw(pos, cell);
    }

    fn rect(&self) -> Rect<usize, usize> {
        self.rect
    }

    fn size(&self) -> Extent2<usize> {
        self.rect.extent()
    }

    fn fg(&self) -> Color { self.fg }
    fn bg(&self) -> Color { self.bg }
    fn attr(&self) -> Attr { self.attr }
}

impl Canvas for Display {
    fn set_cursor_raw(&mut self, pos: Option<Vec2<usize>>) {
        self.set_cursor(pos);
    }

    fn set_raw(&mut self, pos: Vec2<usize>, cell: Cell) {
        self.grid_mut().set(pos, cell);
    }

    fn rect(&self) -> Rect<usize, usize> {
        Rect::new(0, 0, self.size().w, self.size().h)
    }

    fn size(&self) -> Extent2<usize> {
        self.size().into()
    }

    fn fg(&self) -> Color { Color::Reset }
    fn bg(&self) -> Color { Color::Reset }
    fn attr(&self) -> Attr { Attr::Reset }
}
