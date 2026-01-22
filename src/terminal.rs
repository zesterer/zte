use crate::{Error, theme};

use alacritty_terminal::term::cell::Flags;
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    c: char,
    fg: Color,
    bg: Color,
    uline: Color,
    attr: u16,
}

#[derive(Copy, Clone)]
struct CellEntry(u16);

impl CellEntry {
    fn to_cell(&self, fb: &Framebuffer) -> Cell {
        if self.0 & 1 == 0 {
            Cell {
                c: char::from_u32((self.0 as u32 >> 1) & 0b11_1111_1111).unwrap(),
                fg: fb.fg_cache[self.0 as usize >> 11],
                bg: Color::Reset,
                uline: Color::Reset,
                attr: Flags::empty().bits(),
            }
        } else {
            fb.cell_cache[self.0 as usize >> 1]
        }
    }
}

impl Cell {
    fn attr(&self) -> Attributes {
        flags_to_attr(Flags::from_bits(self.attr).unwrap())
    }

    fn insert_into(&self, fb: &mut Framebuffer) -> CellEntry {
        let Self {
            c,
            fg,
            bg,
            uline,
            attr,
        } = self;
        if (0..(1 << 10)).contains(&(*c as u32))
            && *bg == Color::Reset
            && *uline == Color::Reset
            && *attr == Flags::empty().bits()
        {
            let fg = fb.fg_cache.iter().position(|e| e == fg).unwrap_or_else(|| {
                fb.fg_cache.push(*fg);
                fb.fg_cache.len() - 1
            });
            CellEntry(((*c as u16) << 1) | (fg as u16) << 11)
        } else {
            let idx = fb
                .cell_cache
                .iter()
                .position(|e| e == self)
                .unwrap_or_else(|| {
                    fb.cell_cache.push(*self);
                    fb.cell_cache.len() - 1
                });
            CellEntry(1 | (idx as u16) << 1)
        }
    }
}

fn to_ansi_color(col: Color) -> u8 {
    match col {
        Color::Reset => 0,
        Color::Black => 16,
        Color::DarkGrey => 8,
        Color::Red => 9,
        Color::DarkRed => 1,
        Color::Green => 10,
        Color::DarkGreen => 2,
        Color::Yellow => 11,
        Color::DarkYellow => 3,
        Color::Blue => 12,
        Color::DarkBlue => 4,
        Color::Magenta => 13,
        Color::DarkMagenta => 5,
        Color::Cyan => 14,
        Color::DarkCyan => 6,
        Color::White => 15,
        Color::Grey => 7,
        Color::AnsiValue(x) => x,
        Color::Rgb { .. } => 0,
    }
}
fn from_ansi_color(col: u8) -> Color {
    match col {
        0 => Color::Reset,
        col => Color::AnsiValue(col),
    }
}

fn flags_to_attr(flags: Flags) -> Attributes {
    let mut attr = Attributes::none();
    if flags.contains(Flags::INVERSE) {
        attr.set(Attribute::Reverse);
    }
    if flags.contains(Flags::BOLD) {
        attr.set(Attribute::Bold);
    }
    if flags.contains(Flags::ITALIC) {
        attr.set(Attribute::Italic);
    }
    if flags.contains(Flags::UNDERLINE) {
        attr.set(Attribute::Underlined);
    }
    if flags.contains(Flags::DIM) {
        attr.set(Attribute::Dim);
    }
    if flags.contains(Flags::STRIKEOUT) {
        attr.set(Attribute::CrossedOut);
    }
    if flags.contains(Flags::DOUBLE_UNDERLINE) {
        attr.set(Attribute::DoubleUnderlined);
    }
    if flags.contains(Flags::UNDERCURL) {
        attr.set(Attribute::Undercurled);
    }
    if flags.contains(Flags::DOTTED_UNDERLINE) {
        attr.set(Attribute::Underdotted);
    }
    if flags.contains(Flags::DASHED_UNDERLINE) {
        attr.set(Attribute::Underdashed);
    }
    attr
}

fn attr_to_flags(attr: Attributes) -> Flags {
    let mut flags = Flags::empty();
    if attr.has(Attribute::Reverse) {
        flags.set(Flags::INVERSE, true);
    }
    if attr.has(Attribute::Bold) {
        flags.set(Flags::BOLD, true);
    }
    if attr.has(Attribute::Italic) {
        flags.set(Flags::ITALIC, true);
    }
    if attr.has(Attribute::Underlined) {
        flags.set(Flags::UNDERLINE, true);
    }
    if attr.has(Attribute::Dim) {
        flags.set(Flags::DIM, true);
    }
    if attr.has(Attribute::CrossedOut) {
        flags.set(Flags::STRIKEOUT, true);
    }
    if attr.has(Attribute::DoubleUnderlined) {
        flags.set(Flags::DOUBLE_UNDERLINE, true);
    }
    if attr.has(Attribute::Undercurled) {
        flags.set(Flags::UNDERCURL, true);
    }
    if attr.has(Attribute::Underdotted) {
        flags.set(Flags::DOTTED_UNDERLINE, true);
    }
    if attr.has(Attribute::Underdashed) {
        flags.set(Flags::DASHED_UNDERLINE, true);
    }
    flags
}

impl Cell {
    fn apply(&mut self, c: char, theme: theme::CellTheme) {
        self.c = c;
        self.fg = theme.fg.unwrap_or(Color::Reset);
        self.bg = theme.bg.unwrap_or(Color::Reset);
        self.uline = Color::Reset;
        self.attr = attr_to_flags(theme.attr.unwrap_or(Attributes::none())).bits();
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
            uline: Color::Reset,
            attr: attr_to_flags(Attributes::none().with(Attribute::Reset)).bits(),
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
    fn set(&mut self, pos: [usize; 2], c: char, theme: theme::CellTheme) {
        if pos[0] < self.size()[0] && pos[1] < self.size()[1] {
            let offs = [
                self.area.origin[0] as usize + pos[0],
                self.area.origin[1] as usize + pos[1],
            ];
            let mut cell = Cell::default();
            cell.apply(c, theme);
            let entry = cell.insert_into(&mut self.fb);
            self.fb.cells[offs[1] * self.fb.size[0] as usize + offs[0]] = entry;
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

    pub fn mid(&mut self, size: [usize; 2]) -> Rect<'_> {
        let sz = [self.size()[0].min(size[0]), self.size()[1].min(size[1])];
        self.rect(
            [
                self.size()[0] / 2 - sz[0] / 2,
                self.size()[1] / 2 - sz[1] / 2,
            ],
            sz,
        )
    }

    pub fn with_border(&mut self, theme: &theme::BorderTheme, title: Option<&str>) -> Rect<'_> {
        let edge = self.size().map(|e| e.saturating_sub(1));
        for col in 0..edge[0] {
            self.set([col, 0], theme.top, theme.edge);
            if theme.has_edges {
                self.set([col, edge[1]], theme.bottom, theme.edge);
            }
        }
        if theme.has_edges {
            for row in 0..edge[1] {
                self.set([0, row], theme.left, theme.edge);
                self.set([edge[0], row], theme.right, theme.edge);
            }
            self.set([0, edge[1]], theme.bottom_left, theme.edge);
            self.set([edge[0], edge[1]], theme.bottom_right, theme.edge);
        }
        self.set([0, 0], theme.top_left, theme.edge);
        self.set([edge[0], 0], theme.top_right, theme.edge);
        if let Some(title) = title {
            for (i, (c, theme)) in [theme.join_right, ' ']
                .into_iter()
                .map(|c| (c, &theme.edge))
                .chain(title.chars().map(|c| (c, &theme.title)))
                .chain([' ', theme.join_left].into_iter().map(|c| (c, &theme.edge)))
                .enumerate()
            {
                self.set([2 + i, 0], c, *theme);
            }
        }
        if theme.has_edges {
            self.rect([1, 1], self.size().map(|e| e.saturating_sub(2)))
        } else {
            self.rect([0, 1], self.size().map(|e| e.saturating_sub(1)))
        }
    }

    pub fn with_fg(&mut self, fg: impl Into<Option<Color>>) -> Rect<'_> {
        let fg = fg.into();
        Rect {
            fg: fg.unwrap_or(self.fg),
            ..self.reborrow()
        }
    }

    pub fn with_bg(&mut self, bg: impl Into<Option<Color>>) -> Rect<'_> {
        let bg = bg.into();
        Rect {
            bg: bg.unwrap_or(self.bg),
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
                // TODO: uline
                self.set(
                    [col, row],
                    c,
                    theme::CellTheme {
                        fg: Some(self.fg),
                        bg: Some(self.bg),
                        attr: Some(self.attr),
                    },
                );
            }
        }
        self.reborrow()
    }

    pub fn text(&mut self, origin: [isize; 2], text: &str) -> Rect<'_> {
        for (idx, c) in text.chars().enumerate() {
            if (0..self.size()[0] as isize).contains(&(origin[0] + idx as isize)) && origin[1] >= 0
            {
                // TODO: uline
                self.set(
                    [(origin[0] + idx as isize) as usize, origin[1] as usize],
                    *c.borrow(),
                    theme::CellTheme {
                        fg: Some(self.fg),
                        bg: Some(self.bg),
                        // Apply dimming to all unfocused things
                        attr: Some(
                            self.attr
                                | if self.has_focus {
                                    Attributes::none()
                                } else {
                                    Attributes::none().with(Attribute::Dim)
                                },
                        ),
                    },
                );
            }
        }
        self.reborrow()
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
        self.reborrow()
    }

    pub fn hide_cursor(&mut self) -> Rect<'_> {
        self.fb.cursor = None;
        self.reborrow()
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
    cells: Vec<CellEntry>,
    cursor: Option<([u16; 2], CursorStyle)>,
    title: String,
    bell: bool,
    fg_cache: Vec<Color>,
    cell_cache: Vec<Cell>,
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
            self.fb[0]
                .cells
                .resize(self.size[0] as usize * self.size[1] as usize, CellEntry(1));
            self.fb[0].fg_cache.resize(1, Color::Reset);
            self.fb[0].cell_cache.clear();
        }
        self.fb[0].cursor = None;

        self.fb[0].fg_cache.clear();
        self.fb[0].cell_cache.clear();

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
                        let cell = self.fb[0].cells[pos].to_cell(&self.fb[0]);

                        let changed = self.fb[0].size != self.fb[1].size
                            || cell != self.fb[1].cells[pos].to_cell(&self.fb[1]);

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
                            if attr != cell.attr() {
                                attr = cell.attr();
                                stdout
                                    .queue(style::SetAttributes(
                                        Attributes::none().with(Attribute::Reset),
                                    ))
                                    .unwrap();
                                stdout.queue(style::SetAttributes(attr)).unwrap();
                                stdout.queue(style::SetForegroundColor(fg)).unwrap();
                                stdout.queue(style::SetBackgroundColor(bg)).unwrap();
                                stdout.queue(style::SetUnderlineColor(uline)).unwrap();
                            }

                            // Convert non-printable chars
                            let c = match cell.c {
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
