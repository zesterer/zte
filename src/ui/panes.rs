use super::*;
use crate::{
    state::{BufferId, Cursor, CursorId},
    terminal::CursorStyle,
};

#[derive(Clone)]
pub struct Doc {
    buffer: BufferId,
    cursor: CursorId,
}

impl Doc {
    pub fn new(state: &mut State, buffer: BufferId) -> Self {
        Self {
            buffer,
            cursor: state.buffers[buffer].cursors.insert(Cursor::default()),
        }
    }
}

impl Element for Doc {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };
        let Some(cursor) = buffer.cursors.get(self.cursor) else {
            return Err(event);
        };

        match event.to_action(|e| {
            e.to_char()
                .map(Action::Char)
                .or_else(|| e.to_move().map(Action::Move))
                .or_else(|| e.to_pane_move().map(Action::PaneMove))
        }) {
            Some(Action::Char(c)) => {
                buffer.insert(cursor.pos, c);
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        }
    }
}

impl Visual for Doc {
    fn render(&self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(self.buffer) else {
            return;
        };
        let Some(cursor) = buffer.cursors.get(self.cursor) else {
            return;
        };

        let mut n = 0;
        for (i, line) in buffer.chars.split(|c| *c == '\n').enumerate() {
            frame.text([0, i], line);

            if (n..=n + line.len()).contains(&cursor.pos) {
                frame.set_cursor([cursor.pos - n, i], CursorStyle::BlinkingBar);
            }

            n += line.len() + 1;
        }
    }
}

#[derive(Clone)]
pub enum Pane {
    Doc(Doc),
}

pub struct Panes {
    selected: usize,
    panes: Vec<Pane>,
}

impl Panes {
    pub fn new(state: &mut State, buffers: &[BufferId]) -> Self {
        Self {
            selected: 0,
            panes: buffers
                .iter()
                .map(|b| Pane::Doc(Doc::new(state, *b)))
                .collect(),
        }
    }
}

impl Element for Panes {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        match event.to_action(|e| e.to_pane_move().map(Action::PaneMove)) {
            Some(Action::PaneMove(Dir::Left)) => {
                self.selected = (self.selected + self.panes.len() - 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Right)) => {
                self.selected = (self.selected + 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            err => {
                if let Some(pane) = self.panes.get_mut(self.selected) {
                    // Pass to pane
                    match pane {
                        Pane::Doc(doc) => doc.handle(state, event),
                    }
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        }
    }
}

impl Visual for Panes {
    fn render(&self, state: &State, frame: &mut Rect) {
        for (i, pane) in self.panes.iter().enumerate() {
            let boundary = |i| frame.size()[0] * i / self.panes.len();

            let (x0, x1) = (boundary(i), boundary(i + 1));

            let is_selected = self.selected == i;
            let border_theme = if frame.has_focus() && is_selected {
                &state.theme.focus_border
            } else {
                &state.theme.border
            };
            frame
                .rect([x0, 0], [x1 - x0, frame.size()[1]])
                .with_border(border_theme)
                .with_focus(is_selected)
                .with(|frame| match pane {
                    Pane::Doc(doc) => doc.render(state, frame),
                });
        }
    }
}
