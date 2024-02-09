use super::Error;

pub use crossterm::{
    cursor::SetCursorStyle as CursorStyle,
    event::Event as TerminalEvent,
    style::Color,
};

use std::{
    io::{self, StdoutLock, Write as _},
    panic,
    time::Duration,
    borrow::Borrow,
};
use crossterm::{
    event,
    style,
    cursor,
    terminal,
    ExecutableCommand,
    QueueableCommand,
    SynchronizedUpdate,
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

pub struct Rect<'a> {
    fg: Color,
    bg: Color,
    origin: [u16; 2],
    size: [u16; 2],
    fb: &'a mut Framebuffer,
}

impl<'a> Rect<'a> {
    fn get_mut(&mut self, pos: [usize; 2]) -> Option<&mut Cell> {
        if pos[0] < self.size()[0] && pos[1] < self.size()[1] {
            let offs = [self.origin[0] as usize + pos[0], self.origin[1] as usize + pos[1]];
            Some(&mut self.fb.cells[offs[1] * self.fb.size[0] as usize + offs[0]])
        } else {
            None
        }
    }
    
    pub fn with<R>(&mut self, f: impl FnOnce(&mut Rect) -> R) -> R { f(self) }
    
    pub fn rect(&mut self, origin: [usize; 2], size: [usize; 2]) -> Rect {
        Rect {
            origin: [self.origin[0] + origin[0] as u16, self.origin[1] + origin[1] as u16],
            size: [
                size[0].min((self.size[0] as usize).saturating_sub(origin[0])) as u16,
                size[1].min((self.size[1] as usize).saturating_sub(origin[1])) as u16,
            ],
            fg: self.fg,
            bg: self.bg,
            fb: self.fb,
        }
    }
    
    pub fn with_fg(&mut self, fg: Color) -> Rect {
        Rect { fg, bg: self.bg, origin: self.origin, size: self.size, fb: self.fb }
    }
    
    pub fn with_bg(&mut self, bg: Color) -> Rect {
        Rect { fg: self.fg, bg, origin: self.origin, size: self.size, fb: self.fb }
    }

    pub fn size(&self) -> [usize; 2] { self.size.map(|e| e as usize) }
    
    pub fn fill(&mut self, c: char) -> &mut Self {
        for row in 0..self.size()[1] {
            for col in 0..self.size()[0] {
                let cell = Cell { c, fg: self.fg, bg: self.bg };
                if let Some(c) = self.get_mut([col, row]) { *c = cell; }
            }
        }
        self
    }
    
    pub fn text<C: Borrow<char>>(&mut self, origin: [usize; 2], text: impl IntoIterator<Item = C>) -> &mut Self {
        for (idx, c) in text.into_iter().enumerate() {
            if origin[0] + idx >= self.size()[0] {
                break;
            } else {
                let cell = Cell { c: *c.borrow(), fg: self.fg, bg: self.bg };
                if let Some(c) = self.get_mut([origin[0] + idx, origin[1]]) { *c = cell; }
            }
        }
        self
    }
    
    pub fn set_cursor(&mut self, cursor: [usize; 2], style: CursorStyle) -> &mut Self {
        self.fb.cursor = Some((
            [self.origin[0] + cursor[0] as u16, self.origin[1] + cursor[1] as u16],
            style,
        ));
        self
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
            origin: [0, 0],
            size: self.size,
            fb: self,
        }
    }
}

pub struct Terminal<'a> {
    stdout: StdoutLock<'a>,
    size: [u16; 2],
    fb: [Framebuffer; 2],
}

impl<'a> Terminal<'a> {
    fn enter(mut stdout: impl io::Write) {
        let _ = terminal::enable_raw_mode();
        let _ = stdout.execute(terminal::EnterAlternateScreen);
    }
    
    fn leave(mut stdout: impl io::Write) {
        let _ = terminal::disable_raw_mode();
        let _ = stdout.execute(terminal::LeaveAlternateScreen);
        let _ = stdout.execute(cursor::Show);
    }
    
    pub fn with<T>(f: impl FnOnce(&mut Self) -> Result<T, Error> + panic::UnwindSafe) -> Result<T, Error> {
        let size = terminal::window_size()?;
        
        Self::enter(io::stdout().lock());
        
        let mut this = Self {
            stdout: io::stdout().lock(),
            size: [size.columns, size.rows],
            fb: [Framebuffer::default(), Framebuffer::default()],
        };
        
        let hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| { Self::leave(io::stdout().lock()); hook(panic); }));
        let res = f(&mut this);
        
        Self::leave(io::stdout().lock());
        
        res
    }
    
    pub fn set_size(&mut self, size: [u16; 2]) {
        self.size = size;
    }
    
    pub fn update(&mut self, render: impl FnOnce(&mut Rect)) {
        // Reset framebuffer
        if self.fb[0].size != self.size {
            self.fb[0].size = self.size;
            self.fb[0].cells.resize(self.size[0] as usize * self.size[1] as usize, Cell::default());
        }
        self.fb[0].cursor = None;
        
        render(&mut self.fb[0].rect());
        
        self.stdout.sync_update(|stdout| {
            let mut cursor_pos = [0, 0];
            let mut fg = Color::Reset;
            let mut bg = Color::Reset;
            stdout
                .queue(cursor::MoveTo(cursor_pos[0], cursor_pos[1])).unwrap()
                .queue(style::SetForegroundColor(fg)).unwrap()
                .queue(style::SetBackgroundColor(bg)).unwrap()
                .queue(cursor::Hide).unwrap();
            
            // Write out changes
            for row in 0..self.size[1] {
                for col in 0..self.size[0] {
                    let pos = row as usize * self.size[0] as usize + col as usize;
                    let cell = self.fb[0].cells[pos];
                    
                    let changed = self.fb[0].size != self.fb[1].size
                        || cell != self.fb[1].cells[pos];
                    
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
                        
                        stdout.queue(style::Print(self.fb[0].cells[pos].c)).unwrap();
                        
                        // Move cursor
                        cursor_pos[0] += 1;
                        if cursor_pos[0] >= self.size[0] { cursor_pos = [0, cursor_pos[1] + 1]; }
                    }
                }
            }
            
            if let Some(([col, row], style)) = self.fb[0].cursor {
                stdout
                    .queue(cursor::MoveTo(col, row)).unwrap()
                    .queue(style).unwrap()
                    .queue(cursor::Show).unwrap();
            } else {
                stdout.queue(cursor::Hide).unwrap();
            }
        }).unwrap();
        
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
