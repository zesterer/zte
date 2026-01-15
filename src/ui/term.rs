use super::*;
use alacritty_terminal::{
    Term as Alacritty,
    term::{Config as AlacrittyConfig, test::TermSize},
    event::VoidListener,
    index::{Point, Line, Column},
};

pub struct Term {
    term: Alacritty<VoidListener>,
}

impl Term {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            term: Alacritty::new(
                AlacrittyConfig::default(),
                &TermSize::new(40, 15),
                VoidListener,
            ),
        }
    }
    
    pub fn close(self, state: &mut State) {
        // Nothing
    }
}

impl Element for Term {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        match event.to_action(|_| None) {
            _ => Err(event),
        }
    }
}

impl Visual for Term {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame
            .with_border(if frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            }, None)
            .with(|frame| {
                for cell self.term.grid().display_iter() {
                    let cell = &self.term.grid()[Point::new(Line(j as i32), Column(i))];
                    frame
                        // .with_bg(cell.bg)
                        // .with_fg(cell.fg)
                        .text([i as isize, j as isize], cell.cell.c.encode_utf8(&mut [0; 4]));
                }
                for j in 0..frame.size()[1] {
                    for i in 0..frame.size()[0] {
                        if self.term.grid().size
                    }
                }
            });
    }
}
