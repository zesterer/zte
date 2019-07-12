use vek::*;
use super::{
    Element,
    Editor,
    Context,
};
use crate::Canvas;

pub enum Tile {
    Editor(Editor),
}

pub struct Column {
    active_tile: usize,
    tiles: Vec<Tile>,
}

impl Column {
    pub fn empty(n: usize) -> Self {
        assert!(n > 0);
        Self {
            active_tile: 0,
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
}

impl Default for Column {
    fn default() -> Self {
        Self::empty(1)
    }
}

impl Element for Column {
    fn render(&self, ctx: Context, canvas: &mut impl Canvas) {
        for (idx, tile) in self.tiles.iter().enumerate() {
            let tile_area = self.tile_area(canvas.size(), idx);
            match tile {
                Tile::Editor(editor) => editor.render(ctx, &mut canvas.window(tile_area)),
            }
        }
    }
}

pub struct Panels {
    active_column: usize,
    columns: Vec<Column>,
}

impl Panels {
    pub fn empty(n: usize) -> Self {
        assert!(n > 0);
        Self {
            active_column: 0,
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
}

impl Default for Panels {
    fn default() -> Self {
        Self::empty(1)
    }
}

impl Element for Panels {
    fn render(&self, ctx: Context, canvas: &mut impl Canvas) {
        for (idx, column) in self.columns.iter().enumerate() {
            column.render(ctx, &mut canvas.window(self.column_area(canvas.size(), idx)));
        }
    }
}
