use vek::*;
use crossterm::{
    AlternateScreen,
    Crossterm,
    TerminalInput,
    ClearType,
};

// Reexports
pub use crossterm::{Color, Attribute as Attr};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cell(pub char, pub Color, pub Color, pub Attr);

impl From<char> for Cell {
    fn from(c: char) -> Self {
        Self(c, Color::Reset, Color::Reset, Attr::Reset)
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::from(' ')
    }
}

#[derive(Clone)]
pub struct Grid {
    size: Extent2<usize>,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(size: Extent2<usize>) -> Self {
        Self {
            size,
            cells: vec![Cell::default(); size.w * size.h],
        }
    }

    pub fn size(&self) -> Extent2<usize> {
        self.size
    }

    fn idx_of(&self, pos: Vec2<usize>) -> usize {
        self.size.w * pos.y + pos.x
    }

    pub fn get(&self, pos: impl Into<Vec2<usize>>) -> Cell {
        self.cells
            .get(self.idx_of(pos.into()))
            .copied()
            .unwrap_or(Cell::default())
    }

    pub fn set(&mut self, pos: impl Into<Vec2<usize>>, cell: Cell) {
        let idx = self.idx_of(pos.into());
        self.cells
            .get_mut(idx)
            .map(|c| *c = cell);
    }
}

pub struct Display {
    size: Extent2<usize>,
    cursor_pos: Option<Vec2<usize>>,
    alt_screen: AlternateScreen,
    grids: (Grid, Grid),
    term: Crossterm,
}

impl Display {
    pub fn new() -> Self {
        let term = Crossterm::new();
        let size = Extent2::from(term.terminal().terminal_size()).map(|e: u16| e as usize);
        let grid = Grid::new(size);
        let mut this = Self {
            size,
            cursor_pos: None,
            alt_screen: AlternateScreen::to_alternate(true).unwrap(),
            grids: (grid.clone(), grid),
            term,
        };
        this.init();
        this
    }

    fn init(&mut self) {
        self.term.terminal().clear(ClearType::All).unwrap();
        self.term.cursor().hide().unwrap();
        for row in 0..self.size.h {
            self.term.cursor().goto(0, row as u16).unwrap();
            for col in 0..self.size.w {
                let Cell(c, fg, bg, attr) = self.grids.0.get((col, row));
                self.term.terminal().write(crossterm::style(c).with(fg).on(bg).attr(attr)).unwrap();
            }
        }
    }

    pub fn input(&self) -> TerminalInput {
        self.term.input()
    }

    #[allow(dead_code)]
    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grids.1
    }

    #[allow(dead_code)]
    pub fn size(&self) -> Extent2<usize> {
        self.size
    }

    #[allow(dead_code)]
    pub fn set_cursor(&mut self, pos: Option<Vec2<usize>>) {
        self.cursor_pos = pos;
    }

    #[allow(dead_code)]
    pub fn render(&mut self) {
        self.term.cursor().goto(0, 0).unwrap();
        let mut last_pos = Vec2::zero();

        for row in 0..self.size.h {
            for col in 0..self.size.w {
                let (front, back) = (self.grids.0.get((col, row)), self.grids.1.get((col, row)));

                if front != back {
                    if last_pos != Vec2::new(col.saturating_sub(1), row) {
                        self.term.cursor().goto(col as u16, row as u16).unwrap();
                        last_pos = Vec2::new(col, row);
                    }

                    let Cell(c, fg, bg, attr) = back;
                    self.term.terminal().write(crossterm::style(c).with(fg).on(bg).attr(attr)).unwrap();
                }
            }
        }

        self.grids.0 = self.grids.1.clone();

        if let Some(cursor_pos) = self.cursor_pos {
            self.term.cursor().show().unwrap();
            self.term.cursor().goto(cursor_pos.x as u16, cursor_pos.y as u16).unwrap();
        } else {
            self.term.cursor().hide().unwrap();
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        self.term.cursor().show().unwrap();
        self.alt_screen.to_main().unwrap();
    }
}
