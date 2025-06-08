use super::*;
use crate::{
    state::{Buffer, CursorId},
    terminal::CursorStyle,
};

#[derive(Copy, Clone, Default)]
enum Mode {
    #[default]
    Doc,
    Prompt,
}

#[derive(Clone, Default)]
pub struct Input {
    pub mode: Mode,
    // x/y location in the buffer that the pane is trying to focus on
    pub focus: [isize; 2],
    // Remember the last known size for things like scrolling
    pub last_size: [usize; 2],
}

impl Input {
    pub fn prompt() -> Self {
        Self {
            mode: Mode::Prompt,
            ..Self::default()
        }
    }

    pub fn refocus(&mut self, buffer: &mut Buffer, cursor_id: CursorId) {
        let Some(cursor) = buffer.cursors.get(cursor_id) else {
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

    pub fn handle(
        &mut self,
        buffer: &mut Buffer,
        cursor_id: CursorId,
        event: Event,
    ) -> Result<Resp, Event> {
        match event.to_action(|e| e.to_char().map(Action::Char).or_else(|| e.to_move())) {
            Some(Action::Char(c)) => {
                if c == '\x08' {
                    buffer.backspace(cursor_id);
                } else if c == '\x7F' {
                    buffer.delete(cursor_id);
                } else {
                    buffer.enter(cursor_id, c);
                }
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Move(dir, page, retain_base)) => {
                let dist = if page {
                    self.last_size.map(|s| s.saturating_sub(3).max(1))
                } else {
                    [1, 1]
                };
                buffer.move_cursor(cursor_id, dir, dist, retain_base);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        }
    }

    pub fn render(
        &mut self,
        state: &State,
        buffer: &Buffer,
        cursor_id: CursorId,
        frame: &mut Rect,
    ) {
        let title = if let Some(path) = &buffer.path {
            Some(path.display().to_string())
        } else {
            None
        };

        // Add frame
        let mut frame = frame.with_border(
            if frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            },
            title.as_deref(),
        );

        let Some(cursor) = buffer.cursors.get(cursor_id) else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);

        let line_num_w = buffer.text.lines().count().max(1).ilog10() as usize + 1;
        let margin_w = match self.mode {
            Mode::Prompt => 2,
            Mode::Doc => line_num_w + 2,
        };

        self.last_size = [frame.size()[0].saturating_sub(margin_w), frame.size()[1]];

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
            match self.mode {
                Mode::Prompt => frame
                    .rect([0, i], [1, 1])
                    .with_bg(state.theme.margin_bg)
                    .with_fg(state.theme.margin_line_num)
                    .fill(' ')
                    .text([0, 0], ">".chars()),
                Mode::Doc => frame
                    .rect([0, i], [margin_w, 1])
                    .with_bg(state.theme.margin_bg)
                    .with_fg(state.theme.margin_line_num)
                    .fill(' ')
                    .text([1, 0], format!("{:>line_num_w$}", line_num + 1).chars()),
            };

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

// impl Visual for Input {
//     fn render(&mut self, state: &State, frame: &mut Rect) {
//         frame.with(|frame| {
//             frame.fill(' ');
//             frame.text([0, 0], self.preamble.chars());

//             frame
//                 .rect([self.preamble.chars().count(), 0], frame.size())
//                 .with(|frame| {
//                     frame.text([0, 0], &self.text);
//                     frame.set_cursor([self.cursor as isize, 0], CursorStyle::BlinkingBar);
//                 });
//         });
//     }
// }
