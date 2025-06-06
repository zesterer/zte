use super::*;
use crate::{
    state::{Buffer, BufferId, Cursor, CursorId},
    terminal::CursorStyle,
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Doc {
    buffer: BufferId,
    // Remember the cursor we use for each buffer
    cursors: HashMap<BufferId, CursorId>,
    // x/y location in the buffer that the pane is trying to focus on
    focus: [isize; 2],
}

impl Doc {
    pub fn new(state: &mut State, buffer: BufferId) -> Self {
        Self {
            buffer,
            // TODO: Don't index directly
            cursors: [(buffer, state.buffers[buffer].start_session())]
                .into_iter()
                .collect(),
            focus: [0, 0],
        }
    }

    fn refocus(&mut self, state: &mut State) {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return;
        };
        let Some(cursor) = buffer.cursors.get(self.cursors[&self.buffer]) else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);
        self.focus[0] = self.focus[0].clamp(cursor_coord[0] - 20, cursor_coord[0] - 4);
        self.focus[1] = self.focus[1].clamp(cursor_coord[1] - 20, cursor_coord[1] - 4);
    }

    pub fn close(self, state: &mut State) {
        for (buffer, cursor) in self.cursors {
            state.buffers[buffer].end_session(cursor);
        }
    }
}

impl Element for Doc {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };

        match event.to_action(|e| {
            e.to_char()
                .map(Action::Char)
                .or_else(|| e.to_move().map(Action::Move))
                .or_else(|| e.to_pane_move().map(Action::PaneMove))
                .or_else(|| e.to_open_switcher())
        }) {
            action @ Some(Action::OpenSwitcher) => Ok(Resp::handled(action)),
            Some(Action::SwitchBuffer(new_buffer)) => {
                self.buffer = new_buffer;
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                // Start a new cursor session for this buffer if one doesn't exist
                self.cursors
                    .entry(self.buffer)
                    .or_insert_with(|| buffer.start_session());
                self.refocus(state);
                Ok(Resp::handled(None))
            }
            Some(Action::Char(c)) => {
                let Some(cursor) = buffer.cursors.get(self.cursors[&self.buffer]) else {
                    return Err(event);
                };
                if c == '\x08' {
                    buffer.backspace(cursor.pos);
                } else if c == '\x7F' {
                    buffer.delete(cursor.pos);
                } else {
                    buffer.insert(cursor.pos, c);
                }
                self.refocus(state);
                Ok(Resp::handled(None))
            }
            Some(Action::Move(dir)) => {
                buffer.move_cursor(self.cursors[&self.buffer], dir);
                self.refocus(state);
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
        let Some(cursor) = buffer.cursors.get(self.cursors[&self.buffer]) else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);

        let line_num_w = buffer.text.lines().count().ilog10() as usize + 1;

        for (i, (line_num, line)) in buffer
            .text
            .lines()
            .enumerate()
            .skip(self.focus[1].max(0) as usize)
            .enumerate()
            .take(frame.size()[1])
        {
            // Margin
            frame
                .rect([0, i], [line_num_w + 2, 1])
                .with_bg(state.theme.margin_bg)
                .with_fg(state.theme.margin_line_num)
                .fill(' ')
                .text([1, 0], format!("{:>line_num_w$}", line_num + 1).chars());

            // Line
            {
                let mut frame = frame.rect([line_num_w + 2, i], [!0, 1]);
                frame.text([0, 0], line);

                // Set cursor position
                if cursor_coord[1] == line_num as isize {
                    frame.set_cursor([cursor_coord[0], 0], CursorStyle::BlinkingBar);
                }
            }
        }
    }
}

#[derive(Clone)]
pub enum Pane {
    Doc(Doc),
}

impl Pane {
    fn title(&self, state: &State) -> Option<String> {
        match self {
            Self::Doc(doc) => {
                let Some(buffer) = state.buffers.get(doc.buffer) else {
                    return None;
                };
                Some(buffer.path.display().to_string())
            }
        }
    }
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

            // Draw pane contents
            frame
                .rect([x0, 0], [x1 - x0, frame.size()[1]])
                .with_border(border_theme, pane.title(state).as_deref())
                .with_focus(is_selected)
                .with(|frame| match pane {
                    Pane::Doc(doc) => doc.render(state, frame),
                });
        }
    }
}
