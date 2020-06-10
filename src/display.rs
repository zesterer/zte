use std::{
    io::{Write, stdout, Stdout},
    fmt,
};
use vek::*;
use termion::{
    screen::AlternateScreen,
    input::MouseTerminal,
    raw::{RawTerminal, IntoRawMode},
    clear,
    cursor,
    color,
    terminal_size,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Color {
    Rgb(Rgb<u8>),
    Reset,
}

struct Fg<T>(T);
struct Bg<T>(T);

impl fmt::Display for Fg<Color> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Color::Rgb(rgb) => write!(f, "{}", color::Rgb(rgb.r, rgb.g, rgb.b).fg_string())?,
            Color::Reset => write!(f, "{}", color::Reset.fg_str())?,
        }
        Ok(())
    }
}

impl fmt::Display for Bg<Color> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Color::Rgb(rgb) => write!(f, "{}", color::Rgb(rgb.r, rgb.g, rgb.b).bg_string())?,
            Color::Reset => write!(f, "{}", color::Reset.bg_str())?,
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Attr {
    Reset,
}

impl fmt::Display for Attr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

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

    fn idx_of(&self, pos: Vec2<usize>) -> Option<usize> {
        if pos.map2(self.size.into(), |e, sz| e < sz).reduce_and() {
            Some(self.size.w * pos.y + pos.x)
        } else {
            None
        }
    }

    pub fn get(&self, pos: impl Into<Vec2<usize>>) -> Cell {
        match self.idx_of(pos.into()) {
            Some(idx) => self.cells
                .get(idx)
                .copied()
                .unwrap_or(Cell::default()),
            None => Cell::default(),
        }
    }

    pub fn set(&mut self, pos: impl Into<Vec2<usize>>, cell: Cell) {
        match self.idx_of(pos.into()) {
            Some(idx) => {
                self.cells
                    .get_mut(idx)
                    .map(|c| *c = cell);
            },
            None => {},
        }
    }
}

pub struct Display {
    size: Extent2<usize>,
    cursor_pos: Option<Vec2<usize>>,
    grids: (Grid, Grid),
    screen: AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>,
}

impl Display {
    pub fn new() -> Self {
        let screen = AlternateScreen::from(MouseTerminal::from(stdout().into_raw_mode().unwrap()));

        let size = Extent2::from(terminal_size().unwrap()).map(|e: u16| e as usize);
        let grid = Grid::new(size);
        let mut this = Self {
            size,
            cursor_pos: None,
            grids: (grid.clone(), grid),
            screen,
        };
        this.init();
        this
    }

    fn init(&mut self) {
        write!(self.screen, "{}", clear::All).unwrap();
        write!(self.screen, "{}", cursor::Save).unwrap();
        write!(self.screen, "{}", cursor::Hide).unwrap();
        write!(self.screen, "{}", cursor::BlinkingBar).unwrap();
        for row in 0..self.size.h {
            write!(self.screen, "{}", cursor::Goto(1, row as u16 + 1)).unwrap();
            for col in 0..self.size.w {
                let Cell(c, fg, bg, attr) = self.grids.0.get((col, row));
                write!(self.screen, "{}", c).unwrap();
            }
        }
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
        write!(self.screen, "{}", cursor::Goto(1, 1)).unwrap();
        let mut last_pos = Vec2::zero();

        for row in 0..self.size.h {
            for col in 0..self.size.w {
                let (front, back) = (self.grids.0.get((col, row)), self.grids.1.get((col, row)));

                if front != back {
                    if last_pos != Vec2::new(col.saturating_sub(1), row) {
                        write!(self.screen, "{}", cursor::Goto(col as u16 + 1, row as u16 + 1)).unwrap();
                    }

                    let Cell(c, fg, bg, attr) = back;
                    write!(self.screen, "{}{}{}{}", Fg(fg), Bg(bg), attr, c).unwrap();
                    last_pos = Vec2::new(col, row);
                }
            }
        }

        self.grids.0 = self.grids.1.clone();

        if let Some(cursor_pos) = self.cursor_pos {
            write!(self.screen, "{}", cursor::Show).unwrap();
            write!(self.screen, "{}", cursor::Goto(cursor_pos.x as u16 + 1, cursor_pos.y as u16 + 1)).unwrap();
        } else {
            write!(self.screen, "{}", cursor::Hide).unwrap();
        }

        self.screen.flush().unwrap();
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        write!(self.screen, "{}", cursor::Show).unwrap();
        write!(self.screen, "{}", cursor::Restore).unwrap();
    }
}
