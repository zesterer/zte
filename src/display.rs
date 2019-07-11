use std::io::{self, stdout, Stdout};
use vek::*;
use crossterm::{
    AlternateScreen,
    Crossterm,
    TerminalInput,
    ClearType,
};

pub struct Display {
    size: Extent2<u16>,
    alt_screen: AlternateScreen,
    term: Crossterm,
    stdout: Stdout,
}

impl Display {
    pub fn new() -> Self {
        let mut this = Self {
            size: Extent2::zero(),
            alt_screen: AlternateScreen::to_alternate(true).unwrap(),
            term: Crossterm::new(),
            stdout: stdout(),
        };
        this.init();
        this
    }

    fn init(&mut self) {
        self.term.terminal().clear(ClearType::All).unwrap();
        self.size = self.term.terminal().terminal_size().into();
    }

    pub fn input(&self) -> TerminalInput {
        self.term.input()
    }

    #[allow(dead_code)]
    pub fn size(&self) -> Extent2<u16> {
        self.size
    }

    #[allow(dead_code)]
    pub fn show_cursor(&mut self, show: bool) {
        if show {
            self.term.cursor().show().unwrap();
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

impl io::Write for Display {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.stdout.write(b)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
