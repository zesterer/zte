use vek::*;
use super::{
    Element,
    Editor,
    Terminal,
    Context,
};
use crate::{
    Canvas,
    Dir,
    Event,
};

pub enum Tile {
    Editor(Editor),
    Terminal(Terminal),
}

pub struct Column {
    active_idx: usize,
    tiles: Vec<Tile>,
}

impl Column {
    pub fn single(tile: Tile) -> Self {
        Self::many(vec![tile])
    }

    pub fn many(tiles: Vec<Tile>) -> Self {
        assert!(tiles.len() > 0);
        Self {
            active_idx: 0,
            tiles,
        }
    }

    pub fn active_idx(&self) -> usize {
        self.active_idx
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    fn tile_area(&self, size: Extent2<usize>, idx: usize) -> Rect<usize, usize> {
        let avg_h = size.h / self.tiles.len();
        let h = if idx == self.tiles.len() - 1 { size.h - avg_h * idx } else { avg_h };
        Rect::new(0, avg_h * idx, size.w, h)
    }

    pub fn active_mut(&mut self) -> Option<&mut Tile> {
        self.tiles.get_mut(self.active_idx)
    }

    pub fn switch_to(&mut self, idx: isize) -> Result<(), ()> {
        let idx = idx.rem_euclid(self.tiles.len() as isize) as usize;
        if (0..self.tiles.len()).contains(&idx) {
            self.active_idx = idx;
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn close_editor(&mut self) -> bool {
        if self.tiles.len() > 1 {
            self.tiles.remove(self.active_idx);
            self.active_idx = self.active_idx.saturating_sub(1);
            true
        } else {
            false
        }
    }
}

impl Element for Column {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
        match event {
            Event::SwitchEditor(Dir::Up) => { let _ = self.switch_to(self.active_idx as isize - 1); },
            Event::SwitchEditor(Dir::Down) => { let _ = self.switch_to(self.active_idx as isize + 1); },
            Event::NewEditor(Dir::Up) => {
                self.tiles.insert(self.active_idx, Tile::Editor(Editor::recent(ctx)));
            },
            Event::NewEditor(Dir::Down) => {
                self.tiles.insert(self.active_idx + 1, Tile::Editor(Editor::recent(ctx)));
                self.active_idx += 1;
            },
            Event::NewTerminal(Dir::Up) => {
                self.tiles.insert(self.active_idx, Tile::Terminal(Terminal::default()));
            },
            Event::NewTerminal(Dir::Down) => {
                self.tiles.insert(self.active_idx + 1, Tile::Terminal(Terminal::default()));
                self.active_idx += 1;
            },
            event => match self.active_mut() {
                Some(Tile::Editor(editor)) => editor.handle(ctx, event)?,
                Some(Tile::Terminal(terminal)) => terminal.handle(ctx, event),
                None => {},
            },
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        for idx in 0..self.tiles.len() {
            let tile_area = self.tile_area(canvas.size(), idx);
            match &mut self.tiles[idx] {
                Tile::Editor(editor) => editor.update(ctx, &mut canvas.window(tile_area), active && idx == self.active_idx),
                Tile::Terminal(terminal) => terminal.update(ctx, &mut canvas.window(tile_area), active && idx == self.active_idx),
            }
        }
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        for (idx, tile) in self.tiles.iter().enumerate() {
            let tile_area = self.tile_area(canvas.size(), idx);
            match tile {
                Tile::Editor(editor) => editor.render(ctx, &mut canvas.window(tile_area), active && idx == self.active_idx),
                Tile::Terminal(terminal) => terminal.render(ctx, &mut canvas.window(tile_area), active && idx == self.active_idx),
            }
        }
    }
}

pub struct Panels {
    active_idx: usize,
    columns: Vec<Column>,
}

impl Panels {
    pub fn empty(ctx: &mut Context, n: usize) -> Self {
        Self {
            active_idx: 0,
            columns: (0..n)
                .map(|_| Column::single(Tile::Editor(Editor::empty(ctx))))
                .collect(),
        }
    }

    pub fn insert_column(&mut self, idx: usize, tile: Tile) {
        self.columns.insert(idx, Column::single(tile));
    }

    fn column_area(&self, size: Extent2<usize>, idx: usize) -> Rect<usize, usize> {
        let avg_w = size.w / self.columns.len();
        let w = if idx == self.columns.len() - 1 { size.w - avg_w * idx } else { avg_w };
        Rect::new(avg_w * idx, 0, w, size.h)
    }

    pub fn active_mut(&mut self) -> Option<&mut Column> {
        self.columns.get_mut(self.active_idx)
    }

    pub fn switch_to(&mut self, idx: isize) -> Result<(), ()> {
        let vertical = self
            .active_mut()
            .map(|col| col.active_idx() as f32 / col.len() as f32)
            .unwrap_or(0.0);

        let idx = idx.rem_euclid(self.columns.len() as isize) as usize;
        if (0..self.columns.len()).contains(&idx) {
            self.active_idx = idx;
            // Try to choose a tile that's got approximately the same vertical location as the last one
            self
                .active_mut()
                .map(|col| col.switch_to((vertical * (col.len() as f32 + 0.5)).floor() as isize))
                .transpose()?;
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn close_editor(&mut self) -> bool {
        if self.active_mut().map(|col| !col.close_editor()).unwrap_or(true) {
            if self.columns.len() > 1 {
                self.columns.remove(self.active_idx);
                self.active_idx = self.active_idx.saturating_sub(1);
                true
            } else {
                false
            }
        } else {
            true
        }
    }
}

impl Element for Panels {
    type Response = ();

    fn handle(&mut self, ctx: &mut Context, event: Event) {
        match event {
            Event::SwitchEditor(Dir::Left) => { let _ = self.switch_to(self.active_idx as isize - 1); },
            Event::SwitchEditor(Dir::Right) => { let _ = self.switch_to(self.active_idx as isize + 1); },
            Event::NewEditor(Dir::Left) => {
                self.columns.insert(self.active_idx, Column::single(Tile::Editor(Editor::recent(ctx))));
            },
            Event::NewEditor(Dir::Right) => {
                self.columns.insert(self.active_idx + 1, Column::single(Tile::Editor(Editor::recent(ctx))));
                self.active_idx += 1;
            },
            Event::NewTerminal(Dir::Left) => {
                self.columns.insert(self.active_idx, Column::single(Tile::Terminal(Terminal::default())));
            },
            Event::NewTerminal(Dir::Right) => {
                self.columns.insert(self.active_idx + 1, Column::single(Tile::Terminal(Terminal::default())));
                self.active_idx += 1;
            },
            Event::CloseEditor => { let _ = self.close_editor(); },
            event => { self.active_mut().map(|col| col.handle(ctx, event)); },
        }
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        for idx in 0..self.columns.len() {
            let column_area = self.column_area(canvas.size(), idx);
            self.columns[idx].update(ctx, &mut canvas.window(column_area), active && idx == self.active_idx);
        }
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        canvas.rectangle((0, 0), sz, '!');
        for (idx, column) in self.columns.iter().enumerate() {
            column.render(ctx, &mut canvas.window(self.column_area(canvas.size(), idx)), active && idx == self.active_idx);
        }
    }
}
