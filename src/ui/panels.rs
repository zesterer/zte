use vek::*;
use super::{
    Element,
    Editor,
    Context,
};
use crate::{
    Canvas,
    Dir,
    Event,
};

pub enum Tile {
    Editor(Editor),
}

pub struct Column {
    active_idx: usize,
    tiles: Vec<Tile>,
}

impl Column {
    pub fn empty(n: usize) -> Self {
        assert!(n > 0);
        Self {
            active_idx: 0,
            tiles: (0..n)
                .map(|_| Tile::Editor(Editor::default()))
                .collect(),
        }
    }

    fn tile_area(&self, size: Extent2<usize>, idx: usize) -> Rect<usize, usize> {
        let avg_h = size.h / self.tiles.len();
        let h = if idx == self.tiles.len() - 1 { size.h - avg_h * idx } else { avg_h };
        Rect::new(0, avg_h * idx, size.w, h)
    }

    pub fn active_mut(&mut self) -> Option<&mut Tile> {
        self.tiles.get_mut(self.active_idx)
    }

    pub fn switch_to(&mut self, idx: usize) -> Result<(), ()> {
        if (0..self.tiles.len()).contains(&idx) {
            self.active_idx = idx;
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Default for Column {
    fn default() -> Self {
        Self::empty(1)
    }
}

impl Element for Column {
    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::SwitchEditor(Dir::Up) => { let _ = self.switch_to(self.active_idx.saturating_sub(1)); },
            Event::SwitchEditor(Dir::Down) => { let _ = self.switch_to(self.active_idx.saturating_add(1)); },
            event => { self.active_mut().map(|tile| match tile {
                Tile::Editor(editor) => editor.handle(ctx, event),
            }); },
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        for (idx, tile) in self.tiles.iter().enumerate() {
            let tile_area = self.tile_area(canvas.size(), idx);
            match tile {
                Tile::Editor(editor) => editor.render(ctx, &mut canvas.window(tile_area), active && idx == self.active_idx),
            }
        }
    }
}

pub struct Panels {
    active_idx: usize,
    columns: Vec<Column>,
}

impl Panels {
    pub fn empty(n: usize) -> Self {
        assert!(n > 0);
        Self {
            active_idx: 0,
            columns: (0..n)
                .map(|_| Column::default())
                .collect(),
        }
    }

    fn column_area(&self, size: Extent2<usize>, idx: usize) -> Rect<usize, usize> {
        let avg_w = size.w / self.columns.len();
        let w = if idx == self.columns.len() - 1 { size.w - avg_w * idx } else { avg_w };
        Rect::new(avg_w * idx, 0, w, size.h)
    }

    pub fn active_mut(&mut self) -> Option<&mut Column> {
        self.columns.get_mut(self.active_idx)
    }

    pub fn switch_to(&mut self, idx: usize) -> Result<(), ()> {
        if (0..self.columns.len()).contains(&idx) {
            self.active_idx = idx;
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Default for Panels {
    fn default() -> Self {
        Self::empty(1)
    }
}

impl Element for Panels {
    fn handle(&mut self, ctx: Context, event: Event) {
        match event {
            Event::SwitchEditor(Dir::Left) => { let _ = self.switch_to(self.active_idx.saturating_sub(1)); },
            Event::SwitchEditor(Dir::Right) => { let _ = self.switch_to(self.active_idx.saturating_add(1)); },
            event => { self.active_mut().map(|col| col.handle(ctx, event)); },
        }
    }

    fn render(&self, ctx: Context, canvas: &mut impl Canvas, active: bool) {
        for (idx, column) in self.columns.iter().enumerate() {
            column.render(ctx, &mut canvas.window(self.column_area(canvas.size(), idx)), active && idx == self.active_idx);
        }
    }
}
