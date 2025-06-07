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
    // Remember the last known size for things like scrolling
    last_size: [usize; 2],
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
            last_size: [1, 1],
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
        for i in 0..2 {
            self.focus[i] = self.focus[i].clamp(
                cursor_coord[i] - self.last_size[i] as isize + 1,
                cursor_coord[i],
            );
        }
    }

    pub fn close(self, state: &mut State) {
        for (buffer, cursor) in self.cursors {
            let Some(buffer) = state.buffers.get_mut(buffer) else {
                continue;
            };
            buffer.end_session(cursor);
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
                .or_else(|| e.to_move())
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
                let cursor_id = self.cursors[&self.buffer];
                if c == '\x08' {
                    buffer.backspace(cursor_id);
                } else if c == '\x7F' {
                    buffer.delete(cursor_id);
                } else {
                    buffer.enter(cursor_id, c);
                }
                self.refocus(state);
                Ok(Resp::handled(None))
            }
            Some(Action::Move(dir, page, retain_base)) => {
                let dist = if page {
                    self.last_size.map(|s| s.saturating_sub(3).max(1))
                } else {
                    [1, 1]
                };
                buffer.move_cursor(self.cursors[&self.buffer], dir, dist, retain_base);
                self.refocus(state);
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        }
    }
}

impl Visual for Doc {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(self.buffer) else {
            return;
        };
        let Some(cursor) = buffer.cursors.get(self.cursors[&self.buffer]) else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);

        let line_num_w = buffer.text.lines().count().max(1).ilog10() as usize + 1;
        let margin_w = line_num_w + 2;

        self.last_size = [frame.size()[0] - margin_w, frame.size()[1]];

        let mut pos = 0;
        for (i, (line_num, (line_pos, line))) in buffer
            .text
            .lines()
            .map(move |line| {
                let line_pos = pos;
                pos += line.len();
                (line_pos, line)
            })
            .enumerate()
            .skip(self.focus[1].max(0) as usize)
            .enumerate()
            .take(frame.size()[1])
        {
            // Margin
            frame
                .rect([0, i], [margin_w, 1])
                .with_bg(state.theme.margin_bg)
                .with_fg(state.theme.margin_line_num)
                .fill(' ')
                .text([1, 0], format!("{:>line_num_w$}", line_num + 1).chars());

            // Line
            {
                let mut frame = frame.rect([margin_w, i], [!0, 1]);
                for i in 0..frame.size()[0] {
                    let coord = self.focus[0] + i as isize;
                    if (0..line.len() as isize).contains(&coord) {
                        let pos = line_pos + coord as usize;
                        let selected = cursor.selection().map_or(false, |s| s.contains(&pos));
                        let (fg, c) = match line[coord as usize] {
                            '\n' if selected => (state.theme.whitespace, '⮠'),
                            c => (state.theme.text, c),
                        };
                        frame
                            .with_bg(if selected {
                                state.theme.select_bg
                            } else {
                                Color::Reset
                            })
                            .with_fg(fg)
                            .text([i as isize, 0], &[c]);
                    }
                }

                // Set cursor position
                if cursor_coord[1] == line_num as isize {
                    frame.set_cursor(
                        [cursor_coord[0] - self.focus[0], 0],
                        CursorStyle::BlinkingBar,
                    );
                }
            }

            pos += line.len();
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
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let n = self.panes.len();
        let frame_w = frame.size()[0];
        let boundary = |i| frame_w * i / n;

        for (i, pane) in self.panes.iter_mut().enumerate() {
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
