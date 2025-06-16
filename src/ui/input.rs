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
    Filter,
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

    pub fn filter() -> Self {
        Self {
            mode: Mode::Filter,
            ..Self::default()
        }
    }

    pub fn focus(&mut self, coord: [isize; 2]) {
        for i in 0..2 {
            self.focus[i] =
                self.focus[i].clamp(coord[i] - self.last_size[i] as isize + 1, coord[i]);
        }
    }

    pub fn refocus(&mut self, buffer: &mut Buffer, cursor_id: CursorId) {
        let Some(cursor) = buffer.cursors.get(cursor_id) else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);
        self.focus(cursor_coord);
    }

    pub fn handle(
        &mut self,
        buffer: &mut Buffer,
        cursor_id: CursorId,
        event: Event,
    ) -> Result<Resp, Event> {
        match event.to_action(|e| {
            e.to_char()
                .map(Action::Char)
                .or_else(|| e.to_move())
                .or_else(|| e.to_select_token())
                .or_else(|| e.to_indent())
        }) {
            Some(Action::Char(c)) => {
                if c == '\x08' {
                    buffer.backspace(cursor_id);
                } else if c == '\x7F' {
                    buffer.delete(cursor_id);
                } else {
                    buffer.enter(cursor_id, [c]);
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
            Some(Action::Indent(forward)) => {
                buffer.indent(cursor_id, forward);
                Ok(Resp::handled(None))
            }
            Some(Action::GotoLine(line)) => {
                buffer.goto_line_cursor(cursor_id, line);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::SelectToken) => {
                buffer.select_token_cursor(cursor_id);
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        }
    }

    pub fn render(
        &mut self,
        state: &State,
        title: Option<&str>,
        buffer: &Buffer,
        cursor_id: CursorId,
        search: Option<&Search>,
        frame: &mut Rect,
    ) {
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
            Mode::Filter => 0,
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
                Mode::Filter => frame.rect([0, 0], frame.size()),
                Mode::Prompt => frame
                    .rect([0, i], [1, 1])
                    .with_bg(state.theme.margin_bg)
                    .with_fg(state.theme.margin_line_num)
                    .fill(' ')
                    .text([0, 0], ">"),
                Mode::Doc => frame
                    .rect([0, i], [margin_w, 1])
                    .with_bg(state.theme.margin_bg)
                    .with_fg(state.theme.margin_line_num)
                    .fill(' ')
                    .text([1, 0], &format!("{:>line_num_w$}", line_num + 1)),
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
                            c => {
                                if let Some(fg) = buffer
                                    .highlights
                                    .as_ref()
                                    .and_then(|hl| hl.get_at(pos))
                                    .map(|tok| state.theme.token_color(tok.kind))
                                {
                                    (fg, c)
                                } else {
                                    (state.theme.text, c)
                                }
                            }
                        };
                        let bg = if let Some(s) = search {
                            match s.contains(pos) {
                                Some(true) => state.theme.select_bg,
                                Some(false) => state.theme.unfocus_select_bg,
                                None => Color::Reset,
                            }
                        } else if !selected {
                            Color::Reset
                        } else if frame.has_focus() {
                            state.theme.select_bg
                        } else {
                            state.theme.unfocus_select_bg
                        };
                        frame
                            .with_bg(bg)
                            .with_fg(fg)
                            .text([i as isize, 0], c.encode_utf8(&mut [0; 4]));
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
