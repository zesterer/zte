use crate::{Error, theme};

pub use crossterm::{
    clipboard as cb,
    cursor::SetCursorStyle as CursorStyle,
    event::{Event as TerminalEvent, EventStream},
    style::{Attribute, Attributes, Color},
};

use crossterm::{
    ExecutableCommand, QueueableCommand, SynchronizedUpdate, cursor, event, style, terminal,
};
use std::{
    borrow::Borrow,
    io::{self, StdoutLock, Write as _},
    panic,
};

#[derive(Copy, Clone, PartialEq)]
struct Cell {
    c: char,
    fg: Color,
    bg: Color,
    uline: Color,
    attr: Attributes,
}

impl Cell {
    fn apply(&mut self, c: char, theme: theme::CellTheme) {
        self.c = c;
        self.fg = theme.fg.unwrap_or(Color::Reset);
        self.bg = theme.bg.unwrap_or(Color::Reset);
        self.uline = Color::Reset;
        self.attr = theme.attr.unwrap_or(Attributes::none());
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
            uline: Color::Reset,
            attr: Attributes::none().with(Attribute::Reset),
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

    pub fn translate(&self, pos: [isize; 2]) -> [isize; 2] {
        [
            pos[0] - self.origin[0] as isize,
            pos[1] - self.origin[1] as isize,
        ]
    }

    pub fn contains(&self, pos: [isize; 2]) -> Option<[isize; 2]> {
        if (self.origin[0] as isize..self.origin[0] as isize + self.size[0] as isize)
            .contains(&pos[0])
            && (self.origin[1] as isize..self.origin[1] as isize + self.size[1] as isize)
                .contains(&pos[1])
        {
            Some(self.translate(pos))
        } else {
            None
        }
    }
}

pub struct Rect<'a> {
    pub fg: Color,
    pub bg: Color,
    pub uline: Color,
    pub attr: Attributes,
    area: Area,
    fb: &'a mut Framebuffer,
    has_focus: bool,
}

#[allow(dead_code)]
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

    pub fn with<R>(&mut self, f: impl FnOnce(&mut Rect<'_>) -> R) -> R {
        f(self)
    }

    fn reborrow(&mut self) -> Rect<'_> {
        Rect {
            fg: self.fg,
            bg: self.bg,
            uline: self.uline,
            attr: self.attr,
            area: self.area,
            fb: self.fb,
            has_focus: self.has_focus,
        }
    }

    pub fn rect(&mut self, origin: [usize; 2], size: [usize; 2]) -> Rect<'_> {
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
            ..self.reborrow()
        }
    }

    pub fn with_border(&mut self, theme: &theme::BorderTheme, title: Option<&str>) -> Rect<'_> {
        let edge = self.size().map(|e| e.saturating_sub(1));
        for col in 0..edge[0] {
            self.get_mut([col, 0]).map(|c| {
                c.apply(theme.top, theme.cells);
            });
            if theme.edges {
                self.get_mut([col, edge[1]]).map(|c| {
                    c.apply(theme.bottom, theme.cells);
                });
            }
        }
        if theme.edges {
            for row in 0..edge[1] {
                self.get_mut([0, row]).map(|c| {
                    c.apply(theme.left, theme.cells);
                });
                self.get_mut([edge[0], row]).map(|c| {
                    c.apply(theme.right, theme.cells);
                });
            }
            self.get_mut([0, edge[1]]).map(|c| {
                c.apply(theme.bottom_left, theme.cells);
            });
            self.get_mut([edge[0], edge[1]]).map(|c| {
                c.apply(theme.bottom_right, theme.cells);
            });
        }
        self.get_mut([0, 0]).map(|c| {
            c.apply(theme.top_left, theme.cells);
        });
        self.get_mut([edge[0], 0]).map(|c| {
            c.apply(theme.top_right, theme.cells);
        });
        if let Some(title) = title {
            for (i, c) in [theme.join_right, ' ']
                .into_iter()
                .chain(title.chars())
                .chain([' ', theme.join_left])
                .enumerate()
            {
                self.get_mut([2 + i, 0]).map(|cell| {
                    cell.apply(c, theme.cells);
                });
            }
        }
        if theme.edges {
            self.rect([1, 1], self.size().map(|e| e.saturating_sub(2)))
        } else {
            self.rect([0, 1], self.size().map(|e| e.saturating_sub(1)))
        }
    }

    pub fn with_fg(&mut self, fg: Color) -> Rect<'_> {
        Rect {
            fg,
            ..self.reborrow()
        }
    }

    pub fn with_bg(&mut self, bg: Color) -> Rect<'_> {
        Rect {
            bg,
            ..self.reborrow()
        }
    }

    pub fn with_theme(&mut self, theme: impl Into<Option<theme::CellTheme>>) -> Rect<'_> {
        let theme = theme.into();
        Rect {
            fg: theme.and_then(|t| t.fg).unwrap_or(self.fg),
            bg: theme.and_then(|t| t.bg).unwrap_or(self.bg),
            attr: theme.and_then(|t| t.attr).unwrap_or(self.attr),
            ..self.reborrow()
        }
    }

    pub fn with_uline(&mut self, uline: Option<Color>) -> Rect<'_> {
        Rect {
            uline: uline.unwrap_or(self.uline),
            attr: if uline.is_some() {
                self.attr.with(Attribute::Underlined)
            } else {
                self.attr.without(Attribute::Underlined)
            },
            ..self.reborrow()
        }
    }

    pub fn with_attr(&mut self, attr: Attributes) -> Rect<'_> {
        Rect {
            attr: self.attr | attr,
            ..self.reborrow()
        }
    }

    pub fn with_focus(&mut self, focus: bool) -> Rect<'_> {
        Rect {
            has_focus: self.has_focus && focus,
            ..self.reborrow()
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

    pub fn fill(&mut self, c: char) -> Rect<'_> {
        for row in 0..self.size()[1] {
            for col in 0..self.size()[0] {
                let cell = Cell {
                    c,
                    fg: self.fg,
                    bg: self.bg,
                    uline: self.uline,
                    attr: self.attr,
                };
                if let Some(c) = self.get_mut([col, row]) {
                    *c = cell;
                }
            }
        }
        self.rect([0, 0], self.size())
    }

    pub fn text(&mut self, origin: [isize; 2], text: &str) -> Rect<'_> {
        for (idx, c) in text.chars().enumerate() {
            if (0..self.size()[0] as isize).contains(&(origin[0] + idx as isize)) && origin[1] >= 0
            {
                let cell = Cell {
                    c: *c.borrow(),
                    fg: self.fg,
                    bg: self.bg,
                    uline: self.uline,
                    // Apply dimming to all unfocused things
                    attr: self.attr
                        | if self.has_focus {
                            Attributes::none()
                        } else {
                            Attributes::none().with(Attribute::Dim)
                        },
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

    pub fn set_cursor(&mut self, cursor: [isize; 2], style: CursorStyle) -> Rect<'_> {
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

    pub fn set_title(&mut self, title: String) {
        self.fb.title = title;
    }

    pub fn ring_bell(&mut self) {
        self.fb.bell = true;
    }
}

#[derive(Default)]
pub struct Framebuffer {
    size: [u16; 2],
    cells: Vec<Cell>,
    cursor: Option<([u16; 2], CursorStyle)>,
    title: String,
    bell: bool,
}

impl Framebuffer {
    pub fn rect(&mut self) -> Rect<'_> {
        Rect {
            fg: Color::Reset,
            bg: Color::Reset,
            uline: Color::Reset,
            attr: Attributes::none().with(Attribute::Reset),
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
    copy_to_clipboard: Option<String>,
}

impl<'a> Terminal<'a> {
    fn enter(mut stdout: impl io::Write) {
        let _ = terminal::enable_raw_mode();
        let _ = stdout.execute(terminal::EnterAlternateScreen);
        let _ = stdout.execute(terminal::DisableLineWrap);
        let _ = stdout.execute(event::EnableMouseCapture);
        let _ = stdout.execute(event::EnableBracketedPaste);
    }

    fn leave(mut stdout: impl io::Write) {
        let _ = terminal::disable_raw_mode();
        let _ = stdout.execute(terminal::LeaveAlternateScreen);
        let _ = stdout.execute(terminal::EnableLineWrap);
        let _ = stdout.execute(cursor::Show);
        let _ = stdout.execute(event::DisableMouseCapture);
        let _ = stdout.execute(event::DisableBracketedPaste);
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
            copy_to_clipboard: None,
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

    pub fn copy(&mut self, s: &str) {
        self.copy_to_clipboard = Some(s.to_string());
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
                if self.fb[0].bell {
                    self.fb[0].bell = false;
                    stdout.queue(style::Print('\x07')).unwrap();
                }

                if let Some(content) = self.copy_to_clipboard.take() {
                    stdout
                        .queue(cb::CopyToClipboard::to_clipboard_from(content))
                        .unwrap();
                }

                if self.fb[0].title != self.fb[1].title {
                    stdout.queue(terminal::SetTitle(&self.fb[0].title)).unwrap();
                }

                let mut cursor_pos = [0, 0];
                let mut fg = Color::Reset;
                let mut bg = Color::Reset;
                let mut uline = Color::Reset;
                let mut attr = Attributes::none().with(Attribute::Reset);
                stdout
                    .queue(cursor::MoveTo(cursor_pos[0], cursor_pos[1]))
                    .unwrap()
                    .queue(style::SetForegroundColor(fg))
                    .unwrap()
                    .queue(style::SetBackgroundColor(bg))
                    .unwrap()
                    .queue(style::SetUnderlineColor(uline))
                    .unwrap()
                    .queue(style::SetAttributes(attr))
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
                            if uline != cell.uline {
                                uline = cell.uline;
                                stdout.queue(style::SetUnderlineColor(uline)).unwrap();
                            }
                            if attr != cell.attr {
                                attr = cell.attr;
                                stdout.queue(style::SetAttributes(attr)).unwrap();
                                stdout.queue(style::SetForegroundColor(fg)).unwrap();
                                stdout.queue(style::SetBackgroundColor(bg)).unwrap();
                                stdout.queue(style::SetUnderlineColor(uline)).unwrap();
                            }

                            // Convert non-printable chars
                            let c = match self.fb[0].cells[pos].c {
                                c if c.is_whitespace() => ' ',
                                c if c.is_control() => {
                                    char::from_u32(9216 + c as u32).unwrap_or('?')
                                }
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

    pub fn frame(&mut self) -> Rect<'_> {
        self.fb[0].rect()
    }

    pub fn event_stream(&mut self) -> EventStream {
        EventStream::new()
    }
}
