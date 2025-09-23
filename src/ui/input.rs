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
    // Remember the last area for things like scrolling
    pub last_area: Area,
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
            self.focus[i] = self.focus[i]
                .max(coord[i] - self.last_area.size()[i] as isize + 1)
                .min(coord[i]);
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
        buffer.begin_action();
        match event.to_action(|e| {
            e.to_char()
                .map(Action::Char)
                .or_else(|| e.to_move())
                .or_else(|| e.to_pan())
                .or_else(|| e.to_select_token())
                .or_else(|| e.to_select_all())
                .or_else(|| e.to_indent())
                .or_else(|| e.to_mouse(self.last_area))
                .or_else(|| e.to_edit())
        }) {
            Some(Action::Char(c)) => {
                if c == '\x08' {
                    buffer.backspace(cursor_id);
                } else if c == '\x7F' {
                    buffer.delete(cursor_id);
                } else if c == '\n' {
                    buffer.newline(cursor_id);
                } else {
                    buffer.enter(cursor_id, [c]);
                }
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Move(dir, dist, retain_base, word)) => {
                let dist = match dist {
                    Dist::Char => [1, 1],
                    Dist::Page => self.last_area.size().map(|s| s.saturating_sub(3).max(1)),
                    // TODO: Don't just use an arbitrary very large number
                    Dist::Doc => [1_000_000_000; 2],
                };
                buffer.move_cursor(cursor_id, dir, dist, retain_base, word);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Pan(dir, dist)) => {
                let dist = match dist {
                    Dist::Char => [1, 1],
                    Dist::Page => self.last_area.size().map(|s| s.saturating_sub(3).max(1)),
                    // TODO: Don't just use an arbitrary very large number
                    Dist::Doc => [1_000_000_000; 2],
                };
                let dfocus = match dir {
                    Dir::Up => [0, -1],
                    Dir::Down => [0, 1],
                    Dir::Left => [-1, 0],
                    Dir::Right => [1, 0],
                };
                self.focus[0] += dfocus[0] * dist[0] as isize;
                self.focus[1] += dfocus[1] * dist[1] as isize;
                Ok(Resp::handled(None))
            }
            Some(Action::Indent(forward)) => {
                buffer.indent(cursor_id, forward);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::GotoLine(line)) => {
                buffer.goto_cursor(cursor_id, [0, line], true);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::SelectToken) => {
                buffer.select_token_cursor(cursor_id);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::SelectAll) => {
                buffer.select_all_cursor(cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Click, pos, false)) => {
                buffer.goto_cursor(
                    cursor_id,
                    [self.focus[0] + pos[0], self.focus[1] + pos[1]],
                    true,
                );
                Ok(Resp::handled(None))
            }
            Some(
                Action::Mouse(MouseAction::Drag, pos, false)
                | Action::Mouse(MouseAction::Click, pos, true),
            ) => {
                buffer.goto_cursor(
                    cursor_id,
                    [self.focus[0] + pos[0], self.focus[1] + pos[1]],
                    false,
                );
                Ok(Resp::handled(None))
            }
            Some(Action::Undo) => {
                if buffer.undo() {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Event::Bell)))
                }
            }
            Some(Action::Redo) => {
                if buffer.redo() {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Event::Bell)))
                }
            }
            Some(Action::Copy) => {
                if buffer.copy(cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Event::Bell)))
                }
            }
            Some(Action::Cut) => {
                if buffer.cut(cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Event::Bell)))
                }
            }
            Some(Action::Paste) => {
                if buffer.paste(cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Event::Bell)))
                }
            }
            Some(Action::Duplicate) => {
                buffer.duplicate(cursor_id);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Comment) => {
                buffer.comment(cursor_id);
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
        finder: Option<&Finder>,
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

        self.last_area = frame.rect([margin_w, 0], [!0, !0]).area();

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
                    let line_selected = (line_pos..line_pos + line.len()).contains(&cursor.pos);
                    let pos = if i < line.len() {
                        Some(line_pos + coord as usize)
                    } else {
                        None
                    };
                    let selected = cursor
                        .selection()
                        .zip(pos)
                        .map_or(false, |(s, pos)| s.contains(&pos));
                    let (fg, c) = match line.get(coord as usize).copied() {
                        Some('\n') if selected => (state.theme.whitespace, '⮠'),
                        Some(c) => {
                            if let Some(fg) = buffer
                                .highlights
                                .as_ref()
                                .and_then(|hl| hl.get_at(pos?))
                                .map(|tok| state.theme.token_color(tok.kind))
                            {
                                (fg, c)
                            } else {
                                (state.theme.text, c)
                            }
                        }
                        None => (Color::Reset, ' '),
                    };
                    let bg = match finder.map(|s| s.contains(pos?)) {
                        Some(Some(true)) => state.theme.select_bg,
                        Some(Some(false)) => state.theme.search_result_bg,
                        Some(None) if line_selected && frame.has_focus() => {
                            state.theme.line_select_bg
                        }
                        _ => {
                            if selected {
                                if frame.has_focus() {
                                    state.theme.select_bg
                                } else {
                                    state.theme.unfocus_select_bg
                                }
                            } else if line_selected && frame.has_focus() {
                                state.theme.line_select_bg
                            } else {
                                Color::Reset
                            }
                        }
                    };
                    frame
                        .with_bg(bg)
                        .with_fg(fg)
                        .text([i as isize, 0], c.encode_utf8(&mut [0; 4]));
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
