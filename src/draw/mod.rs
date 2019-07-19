use vek::*;
use crate::display::{
    Cell,
    Color,
    Attr,
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
    fn set_cursor(&mut self, pos: Option<Vec2<usize>>);
    fn set(&mut self, pos: Vec2<usize>, cell: Cell);
    fn rect(&self) -> Rect<usize, usize>;
    fn size(&self) -> Extent2<usize>;

    fn fg(&self) -> Color;
    fn bg(&self) -> Color;
    fn attr(&self) -> Attr;

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
            rect.x,
            rect.y,
            rect.w.min(self.rect().w - rect.x),
            rect.h.min(self.rect().h - rect.y),
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
}

pub struct Drawer<'a, D: Canvas> {
    fg: Color,
    bg: Color,
    attr: Attr,
    canvas: &'a mut D,
    rect: Rect<usize, usize>,
}

impl<'a, D: Canvas> Canvas for Drawer<'a, D> {
    fn set_cursor(&mut self, pos: Option<Vec2<usize>>) {
        self.canvas.set_cursor(pos.map(|pos| self.rect().position() + pos.map2(self.size().into(), |e, sz: usize| e.min(sz.saturating_sub(1)))));
    }

    fn set(&mut self, pos: Vec2<usize>, cell: Cell) {
        let pos = pos.map2(self.size().into(), |e, sz: usize| e.min(sz.saturating_sub(1)));
        self.canvas.set(self.rect().position() + pos, cell);
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
    fn set_cursor(&mut self, pos: Option<Vec2<usize>>) {
        self.set_cursor(pos);
    }

    fn set(&mut self, pos: Vec2<usize>, cell: Cell) {
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
