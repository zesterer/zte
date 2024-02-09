use super::*;
use crate::terminal::CursorStyle;

#[derive(Default)]
pub struct Input {
    pub text: Vec<char>,
    pub cursor: usize,
    pub preamble: &'static str,
}

impl Input {
    pub fn get_text(&self) -> String {
        self.text.iter().copied().collect()
    }
}

impl Element for Input {
    fn handle(&mut self, event: Event) -> Result<Resp, Event> {
        match event.to_action(|e| e.to_char().map(Action::Char)
                .or_else(|| e.to_move().map(Action::Move))) {
            Some(Action::Char('\x08')) => {
                self.cursor = self.cursor.saturating_sub(1);
                if self.text.len() > self.cursor {
                    self.text.remove(self.cursor);
                }
                Ok(Resp::handled(None))
            },
            Some(Action::Char(c)) => {
                self.text.insert(self.cursor, c);
                self.cursor += 1;
                Ok(Resp::handled(None))
            },
            Some(Action::Move(Dir::Left)) => {
                self.cursor = self.cursor.saturating_sub(1);
                Ok(Resp::handled(None))
            },
            Some(Action::Move(Dir::Right)) => {
                self.cursor = (self.cursor + 1).min(self.text.len());
                Ok(Resp::handled(None))
            },
            _ => Err(event),
        }
    }
}

impl Visual for Input {    
    fn render(&self, state: &State, frame: &mut Rect) {
        frame.with(|frame| {
            frame.fill(' ');
            frame.text([0, 0], self.preamble.chars());
            
            frame.rect([self.preamble.chars().count(), 0], frame.size()).with(|frame| {
                frame.text([0, 0], &self.text);
                frame.set_cursor([self.cursor, 0], CursorStyle::BlinkingBar);
            });
        });
    }
}
