use crate::{Error, theme};

pub use crossterm::{
    cursor::SetCursorStyle as CursorStyle, event::Event as TerminalEvent, style::Color,
};

use crossterm::{
    ExecutableCommand, QueueableCommand, SynchronizedUpdate, cursor, event, style, terminal,
};
use std::{
    borrow::Borrow,
    io::{self, StdoutLock, Write as _},
    panic,
    time::Duration,
};

#[derive(Copy, Clone, PartialEq)]
struct Cell {
    c: char,
    fg: Color,
    bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
        }
    }
}

/// Represents an area of the terminal window
#[derive(Copy, Clone, Default)]
pub struct Area {
    origin: [u16; 2],
    size: [u16; 2],
}

impl Area {
    pub fn size(&self) -> [usize; 2] {
        self.size.map(|e| e as usize)
    }

    pub fn contains(&self, pos: [isize; 2]) -> Option<[isize; 2]> {
        if (self.origin[0] as isize..self.origin[0] as isize + self.size[0] as isize)
            .contains(&pos[0])
            && (self.origin[1] as isize..self.origin[1] as isize + self.size[1] as isize)
                .contains(&pos[1])
        {
            Some([
                pos[0] - self.origin[0] as isize,
                pos[1] - self.origin[1] as isize,
            ])
        } else {
            None
        }
    }
}

pub struct Rect<'a> {
    fg: Color,
    bg: Color,
    area: Area,
    fb: &'a mut Framebuffer,
    has_focus: bool,
}

impl<'a> Rect<'a> {
    fn get_mut(&mut self, pos: [usize; 2]) -> Option<&mut Cell> {
        if pos[0] < self.size()[0] && pos[1] < self.size()[1] {
            let offs = [
                self.area.origin[0] as usize + pos[0],
                self.area.origin[1] as usize + pos[1],
            ];
            Some(&mut self.fb.cells[offs[1] * self.fb.size[0] as usize + offs[0]])
        } else {
            None
        }
    }

    pub fn with<R>(&mut self, f: impl FnOnce(&mut Rect) -> R) -> R {
        f(self)
    }

    pub fn rect(&mut self, origin: [usize; 2], size: [usize; 2]) -> Rect {
        Rect {
            area: Area {
                origin: [
                    self.area.origin[0] + origin[0] as u16,
                    self.area.origin[1] + origin[1] as u16,
                ],
                size: [
                    size[0].min((self.area.size[0] as usize).saturating_sub(origin[0])) as u16,
                    size[1].min((self.area.size[1] as usize).saturating_sub(origin[1])) as u16,
                ],
            },
            fg: self.fg,
            bg: self.bg,
            fb: self.fb,
            has_focus: self.has_focus,
        }
    }

    pub fn with_border(&mut self, theme: &theme::BorderTheme, title: Option<&str>) -> Rect {
        let edge = self.size().map(|e| e.saturating_sub(1));
        for col in 0..edge[0] {
            self.get_mut([col, 0]).map(|c| {
                c.c = theme.top;
                c.fg = theme.fg;
            });
            self.get_mut([col, edge[1]]).map(|c| {
                c.c = theme.bottom;
                c.fg = theme.fg;
            });
        }
        for row in 0..edge[1] {
            self.get_mut([0, row]).map(|c| {
                c.c = theme.left;
                c.fg = theme.fg;
            });
            self.get_mut([edge[0], row]).map(|c| {
                c.c = theme.right;
                c.fg = theme.fg;
            });
        }
        self.get_mut([0, 0]).map(|c| {
            c.c = theme.top_left;
            c.fg = theme.fg;
        });
        self.get_mut([edge[0], 0]).map(|c| {
            c.c = theme.top_right;
            c.fg = theme.fg;
        });
        self.get_mut([0, edge[1]]).map(|c| {
            c.c = theme.bottom_left;
            c.fg = theme.fg;
        });
        self.get_mut([edge[0], edge[1]]).map(|c| {
            c.c = theme.bottom_right;
            c.fg = theme.fg;
        });
        if let Some(title) = title {
            for (i, c) in [theme.join_right, ' ']
                .into_iter()
                .chain(title.chars())
                .chain([' ', theme.join_left])
                .enumerate()
            {
                self.get_mut([2 + i, 0]).map(|cell| {
                    cell.fg = theme.fg;
                    cell.c = c
                });
            }
        }
        self.rect([1, 1], self.size().map(|e| e.saturating_sub(2)))
    }

    pub fn with_fg(&mut self, fg: Color) -> Rect {
        Rect {
            fg,
            bg: self.bg,
            area: self.area,
            fb: self.fb,
            has_focus: self.has_focus,
        }
    }

    pub fn with_bg(&mut self, bg: Color) -> Rect {
        Rect {
            fg: self.fg,
            bg,
            area: self.area,
            fb: self.fb,
            has_focus: self.has_focus,
        }
    }

    pub fn with_focus(&mut self, focus: bool) -> Rect {
        Rect {
            fg: self.fg,
            bg: self.bg,
            area: self.area,
            fb: self.fb,
            has_focus: self.has_focus && focus,
        }
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    pub fn area(&self) -> Area {
        self.area
    }

    pub fn size(&self) -> [usize; 2] {
        self.area.size.map(|e| e as usize)
    }

    pub fn fill(&mut self, c: char) -> Rect {
        for row in 0..self.size()[1] {
            for col in 0..self.size()[0] {
                let cell = Cell {
                    c,
                    fg: self.fg,
                    bg: self.bg,
                };
                if let Some(c) = self.get_mut([col, row]) {
                    *c = cell;
                }
            }
        }
        self.rect([0, 0], self.size())
    }

    pub fn text(&mut self, origin: [isize; 2], text: &str) -> Rect {
        for (idx, c) in text.chars().enumerate() {
            if (0..self.size()[0] as isize).contains(&(origin[0] + idx as isize)) && origin[1] >= 0
            {
                let cell = Cell {
                    c: *c.borrow(),
                    fg: self.fg,
                    bg: self.bg,
                };
                if let Some(c) =
                    self.get_mut([(origin[0] + idx as isize) as usize, origin[1] as usize])
                {
                    *c = cell;
                }
            }
        }
        self.rect([0, 0], self.size())
    }

    pub fn set_cursor(&mut self, cursor: [isize; 2], style: CursorStyle) -> Rect {
        if self.has_focus
            && (0..=self.size()[0] as isize).contains(&cursor[0])
            && (0..self.size()[1] as isize).contains(&cursor[1])
        {
            self.fb.cursor = Some((
                [
                    self.area.origin[0] + cursor[0] as u16,
                    self.area.origin[1] + cursor[1] as u16,
                ],
                style,
            ));
        }
        self.rect([0, 0], self.size())
    }
}

#[derive(Default)]
pub struct Framebuffer {
    size: [u16; 2],
    cells: Vec<Cell>,
    cursor: Option<([u16; 2], CursorStyle)>,
}

impl Framebuffer {
    pub fn rect(&mut self) -> Rect {
        Rect {
            fg: Color::Reset,
            bg: Color::Reset,
            area: Area {
                origin: [0, 0],
                size: self.size,
            },
            fb: self,
            has_focus: true,
        }
    }
}

pub struct Terminal<'a> {
    stdout: StdoutLock<'a>,
    size: [u16; 2],
    fb: [Framebuffer; 2],
    bell: bool,
}

impl<'a> Terminal<'a> {
    fn enter(mut stdout: impl io::Write) {
        let _ = terminal::enable_raw_mode();
        let _ = stdout.execute(terminal::EnterAlternateScreen);
        let _ = stdout.execute(event::EnableMouseCapture);
    }

    fn leave(mut stdout: impl io::Write) {
        let _ = terminal::disable_raw_mode();
        let _ = stdout.execute(terminal::LeaveAlternateScreen);
        let _ = stdout.execute(cursor::Show);
        let _ = stdout.execute(event::DisableMouseCapture);
    }

    pub fn with<T>(
        f: impl FnOnce(&mut Self) -> Result<T, Error> + panic::UnwindSafe,
    ) -> Result<T, Error> {
        let size = terminal::window_size()?;

        Self::enter(io::stdout().lock());

        let mut this = Self {
            stdout: io::stdout().lock(),
            size: [size.columns, size.rows],
            fb: [Framebuffer::default(), Framebuffer::default()],
            bell: false,
        };

        let hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| {
            Self::leave(io::stdout().lock());
            hook(panic);
        }));
        let res = f(&mut this);

        Self::leave(io::stdout().lock());

        res
    }

    pub fn set_size(&mut self, size: [u16; 2]) {
        self.size = size;
    }

    pub fn ring_bell(&mut self) {
        self.bell = true;
    }

    pub fn update(&mut self, render: impl FnOnce(&mut Rect)) {
        // Reset framebuffer
        if self.fb[0].size != self.size {
            self.fb[0].size = self.size;
            self.fb[0].cells.resize(
                self.size[0] as usize * self.size[1] as usize,
                Cell::default(),
            );
        }
        self.fb[0].cursor = None;

        render(&mut self.fb[0].rect());

        self.stdout
            .sync_update(|stdout| {
                if self.bell {
                    self.bell = false;
                    stdout.queue(style::Print('\x07')).unwrap();
                }

                let mut cursor_pos = [0, 0];
                let mut fg = Color::Reset;
                let mut bg = Color::Reset;
                stdout
                    .queue(cursor::MoveTo(cursor_pos[0], cursor_pos[1]))
                    .unwrap()
                    .queue(style::SetForegroundColor(fg))
                    .unwrap()
                    .queue(style::SetBackgroundColor(bg))
                    .unwrap()
                    .queue(cursor::Hide)
                    .unwrap();

                // Write out changes
                for row in 0..self.size[1] {
                    for col in 0..self.size[0] {
                        let pos = row as usize * self.size[0] as usize + col as usize;
                        let cell = self.fb[0].cells[pos];

                        let changed =
                            self.fb[0].size != self.fb[1].size || cell != self.fb[1].cells[pos];

                        if changed {
                            if cursor_pos != [col, row] {
                                // Minimise the work done to move the cursor around
                                if cursor_pos[1] == row {
                                    stdout.queue(cursor::MoveToColumn(col)).unwrap();
                                } else if cursor_pos[0] == col {
                                    stdout.queue(cursor::MoveToRow(row)).unwrap();
                                } else {
                                    stdout.queue(cursor::MoveTo(col, row)).unwrap();
                                }
                                cursor_pos = [col, row];
                            }
                            if fg != cell.fg {
                                fg = cell.fg;
                                stdout.queue(style::SetForegroundColor(fg)).unwrap();
                            }
                            if bg != cell.bg {
                                bg = cell.bg;
                                stdout.queue(style::SetBackgroundColor(bg)).unwrap();
                            }

                            // Convert non-printable chars
                            let c = match self.fb[0].cells[pos].c {
                                c if c.is_whitespace() => ' ',
                                c => c,
                            };
                            stdout.queue(style::Print(c)).unwrap();

                            // Move cursor
                            cursor_pos[0] +=
                                unicode_display_width::width(c.encode_utf8(&mut [0; 4])) as u16;
                        }
                    }
                }

                if let Some(([col, row], style)) = self.fb[0].cursor {
                    stdout
                        .queue(cursor::MoveTo(col, row))
                        .unwrap()
                        .queue(style)
                        .unwrap()
                        .queue(cursor::Show)
                        .unwrap();
                } else {
                    stdout.queue(cursor::Hide).unwrap();
                }
            })
            .unwrap();

        self.stdout.flush().unwrap();

        // Switch front and back buffers
        self.fb.swap(0, 1);
    }

    // Get the next pending event, if one is available.
    pub fn get_event(&mut self) -> Option<TerminalEvent> {
        if event::poll(Duration::ZERO).ok()? {
            event::read().ok()
        } else {
            None
        }
    }

    // Wait for the given duration or until an event arrives.
    pub fn wait_at_least(&mut self, dur: Duration) {
        event::poll(dur).unwrap();
    }
}
