use super::*;
use crate::{
    highlight::TokenKind,
    state::{Buffer, Clipboard, CursorId},
    terminal::CursorStyle,
    theme::Theme,
};

#[derive(Copy, Clone, Default)]
enum Mode {
    #[default]
    Doc,
    Prompt,
    Filter,
    SearchResult,
}

#[derive(Clone, Default)]
pub struct Input {
    mode: Mode,
    line_offset: usize,
    // x/y location in the buffer that the pane is trying to focus on
    focus: [isize; 2],
    last_area: Area,
    text_area: Area,
    scroller: Scroller,
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

    pub fn search_result(line_offset: usize) -> Self {
        Self {
            mode: Mode::SearchResult,
            line_offset,
            ..Self::default()
        }
    }

    pub fn focus(&mut self, coord: [isize; 2]) {
        if self.text_area.size() == [0, 0] {
            self.focus = coord;
        } else {
            for i in 0..2 {
                self.focus[i] = self.focus[i]
                    .max(coord[i] - self.text_area.size()[i] as isize + 1)
                    .max(0)
                    .min(coord[i]);
            }
        }
    }

    pub fn refocus(&mut self, buffer: &mut Buffer, cursor_id: CursorId) {
        let Some(cursor) = buffer.cursors.get(cursor_id) else {
            return;
        };
        // Try to focus on both base and pos
        for coord in [cursor.base, cursor.pos].map(|p| buffer.text.to_coord(p)) {
            self.focus(coord);
        }
    }

    pub fn handle(
        &mut self,
        clipboard: &mut Clipboard,
        buffer: &mut Buffer,
        cursor_id: CursorId,
        event: Event,
    ) -> Result<Resp, Event> {
        buffer.begin_action();
        let is_doc = matches!(self.mode, Mode::Doc);

        let event = if is_doc {
            match self
                .scroller
                .handle(event, buffer.text.lines().count(), &mut self.focus)
            {
                Ok(resp) => return Ok(resp),
                Err(event) => event,
            }
        } else {
            event
        };

        // if let Event::Raw(ev) = &event { panic!("{ev:?}") }

        match event.to_action(|e| {
            e.to_char()
                .map(Action::Char)
                .or_else(|| e.to_move())
                .or_else(|| e.to_select_block())
                .or_else(|| e.to_select_all())
                .or_else(|| e.to_indent())
                .or_else(|| e.to_edit())
        }) {
            Some(Action::BackspaceWord) => {
                buffer.backspace(cursor_id, true);
                Ok(Resp::handled(None))
            }
            Some(Action::DeleteWord) => {
                buffer.delete(cursor_id, true);
                Ok(Resp::handled(None))
            }
            Some(Action::Char(c)) => {
                if c == '\x08' {
                    buffer.backspace(cursor_id, false);
                } else if c == '\x7F' {
                    buffer.delete(cursor_id, false);
                } else if c == '\n' {
                    buffer.newline(cursor_id);
                } else {
                    buffer.enter(cursor_id, [c]);
                }
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Move(dir, dist, retain_base, word))
                if matches!(dir, Dir::Left | Dir::Right) || is_doc =>
            {
                let dist = match dist {
                    Dist::Char => [1, 1],
                    Dist::Page => self.text_area.size().map(|s| s.saturating_sub(3).max(1)),
                    // TODO: Don't just use an arbitrary very large number
                    Dist::Doc => [1_000_000_000; 2],
                };
                buffer.move_cursor(cursor_id, dir, dist, retain_base, word);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Indent(forward)) => {
                buffer.indent(cursor_id, forward);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::GotoLine(line)) => {
                buffer.goto_cursor(
                    cursor_id,
                    [0, (line - self.line_offset as isize).max(0)],
                    true,
                );
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::SelectBlock) => {
                buffer.select_block_cursor(cursor_id);
                self.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::SelectAll) => {
                buffer.select_all_cursor(cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Click, pos, Modifiers::NONE, _drag_id))
                if self.last_area.contains(pos).is_some() =>
            {
                if let Some(pos) = self.text_area.contains(pos) {
                    let pos = [self.focus[0] + pos[0], self.focus[1] + pos[1]];
                    // If we're already in the right place, select the token instead
                    if let Some(cursor) = buffer.cursors.get(cursor_id)
                        && cursor.selection().is_none()
                        && buffer.text.to_coord(cursor.pos) == pos
                        && let Some(token) = buffer.token_at_coord(pos)
                        && let TokenKind::Url = token.kind
                    {
                        let token_range = token.range.clone();
                        buffer.select_cursor(cursor_id, token_range)
                        // let url = url.iter().copied().collect::<String>();
                        // if let Ok(url) = url.parse() {
                        //     url_open::open(&url);
                        // } else {
                        //     return Ok(Resp::handled(Some(Action::Show(Some(format!("Could not open URL")), format!("`{url}` is not a valid URL")).into())));
                        // }
                    } else if let Some(cursor) = buffer.cursors.get(cursor_id)
                        && cursor.selection().is_none()
                        && buffer.text.to_coord(cursor.pos) == pos
                    {
                        buffer.select_word_cursor(cursor_id);
                    } else {
                        buffer.goto_cursor(cursor_id, pos, true);
                    }
                }
                Ok(Resp::handled(None))
            }
            Some(
                Action::Mouse(MouseAction::Drag, pos, _, _)
                | Action::Mouse(MouseAction::Click, pos, Modifiers::SHIFT | Modifiers::CTRL, _),
            ) if self.last_area.contains(pos).is_some() => {
                let pos = self.text_area.translate(pos);
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
                    Ok(Resp::handled(Some(Action::Bell.into())))
                }
            }
            Some(Action::Redo) => {
                if buffer.redo() {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Action::Bell.into())))
                }
            }
            Some(Action::Copy) => {
                if buffer.copy(clipboard, cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Action::Bell.into())))
                }
            }
            Some(Action::Cut) => {
                if buffer.cut(clipboard, cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Action::Bell.into())))
                }
            }
            Some(Action::Paste) => {
                if buffer.paste(clipboard, cursor_id) {
                    self.refocus(buffer, cursor_id);
                    Ok(Resp::handled(None))
                } else {
                    Ok(Resp::handled(Some(Action::Bell.into())))
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
            Some(Action::DeleteLine) => {
                buffer.delete_line(cursor_id);
                Ok(Resp::handled(None))
            }
            Some(Action::LineMove(dir)) => {
                buffer.line_move(cursor_id, dir);
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        }
    }

    pub fn render(
        &mut self,
        theme: &Theme,
        title: Option<&str>,
        buffer: &mut Buffer,
        cursor_id: CursorId,
        finder: Option<&Finder>,
        outer_frame: &mut Rect,
    ) {
        self.last_area = outer_frame.area();

        // Add frame
        let mut frame = if matches!(self.mode, Mode::SearchResult) {
            outer_frame.rect([0; 2], [!0; 2])
        } else {
            outer_frame.with_border(
                if outer_frame.has_focus() {
                    &theme.focus_border
                } else {
                    &theme.border
                },
                title.as_deref(),
            )
        };

        let (line_num_w, margin_w) = match self.mode {
            Mode::Prompt => (2, 2),
            Mode::Filter => (0, 0),
            Mode::Doc => {
                let line_num_w = (self.line_offset + buffer.text.lines().count())
                    .max(1)
                    .ilog10() as usize
                    + 1;
                (line_num_w, line_num_w + 2)
            }
            Mode::SearchResult => (4, 6),
        };

        self.text_area = frame.rect([margin_w, 0], [!0, !0]).area();

        let Some(cursor) = buffer.cursors.get(cursor_id).copied() else {
            return;
        };
        let cursor_coord = buffer.text.to_coord(cursor.pos);

        let cursor_delim = buffer
            .highlights
            .get_delim_at(|s| (s.start..=s.end).contains(&cursor.pos));
        let cursor_block = buffer.highlights.get_delim_at(|s| {
            (s.start..=s.end).contains(&cursor.pos)
                && buffer.text.to_coord(s.start)[1]
                    != buffer.text.to_coord(s.end.saturating_sub(1))[1]
        });

        let mut pos = 0;
        for (i, (line_num, (line_pos, line))) in buffer
            .text
            .lines()
            .chain(if buffer.text.lines().len() == 0 {
                Some(buffer.text.slice(0..0))
            } else {
                None
            })
            .map(move |line| {
                let line_pos = pos;
                pos += line.byte_len();
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
                    .with_theme(theme.margin)
                    .fill(' ')
                    .text([0, 0], ">"),
                Mode::Doc | Mode::SearchResult => frame
                    .rect([0, i], [margin_w, 1])
                    .with_theme(theme.margin)
                    .fill(' ')
                    .text(
                        [1, 0],
                        &format!("{:>line_num_w$}", self.line_offset + line_num + 1),
                    ),
            };

            let line_highlight_selected = matches!(self.mode, Mode::Doc)
                && buffer.text.to_coord(cursor.pos)[1] == line_num as isize;

            let block_col = cursor_block
                .as_ref()
                .map(|(_, s)| buffer.text.to_coord(s.end.saturating_sub(1))[0]);

            // Line
            {
                let mut frame = frame.rect([margin_w, i], [!0, 1]);
                let mut chars = line.chars();
                let mut pos = line_pos;
                for _ in 0..self.focus[0].max(0) as usize {
                    if let Some(c) = chars.next() {
                        pos += c.len_utf8();
                    }
                }
                let mut pos = Some(pos);
                for i in 0..frame.size()[0] {
                    let coord = self.focus[0] + i as isize;
                    let line_c = chars.next();

                    let selected = cursor
                        .selection()
                        .zip(pos)
                        .map_or(false, |(s, pos)| s.contains(&pos));

                    let (mut frame, c) = match line_c {
                        Some('\n') if selected => (frame.with_theme(theme.whitespace), '⮠'),
                        Some(c) => {
                            if let Some(theme) = pos
                                .and_then(|pos| {
                                    buffer.highlights.get_at(
                                        &buffer.lang.highlighter,
                                        &buffer.text,
                                        pos,
                                    )
                                })
                                .map(|tok| theme.token_theme(tok.kind))
                            {
                                (frame.with_theme(theme), c)
                            } else {
                                (frame.with_theme(None), c)
                            }
                        }
                        None => (frame.with_theme(None), ' '),
                    };
                    let mut frame = match finder.map(|s| s.contains(pos?)) {
                        Some(Some(true)) => frame.with_theme(theme.select),
                        Some(Some(false)) => frame.with_theme(theme.search_result),
                        _ => {
                            if selected {
                                if frame.has_focus() {
                                    frame.with_theme(theme.select)
                                } else {
                                    frame.with_theme(theme.unfocus_select)
                                }
                            } else if frame.bg == Color::Reset
                                && line_highlight_selected
                                && frame.has_focus()
                            {
                                frame.with_theme(theme.line_select)
                            } else {
                                frame.with_theme(None)
                            }
                        }
                    };
                    // Block marker line
                    let (mut frame, c) = if block_col == Some(coord)
                        && c.is_whitespace()
                        && let Some((_, span)) = &cursor_block
                        && span.contains(&(line_pos + coord as usize))
                    {
                        (frame.with_theme(theme.margin), '┆')
                    } else {
                        (frame.with_theme(None), c)
                    };
                    // Matching delimiters
                    let mut frame = if cursor_delim
                        .as_ref()
                        .zip(pos)
                        .map_or(false, |((_, s), pos)| s.start == pos || s.end == pos + 1)
                    {
                        frame.with_uline(Some(Color::White))
                    } else {
                        frame.with_theme(None)
                    };
                    frame.text([i as isize, 0], c.encode_utf8(&mut [0; 4]));

                    pos = pos
                        .zip(line_c)
                        .map(|(p, c)| p + c.len_utf8())
                        .filter(|p| *p < line_pos + line.byte_len());
                }
            }

            // Set cursor position
            frame.set_cursor(
                [
                    margin_w as isize + cursor_coord[0] - self.focus[0],
                    cursor_coord[1] - self.focus[1],
                ],
                CursorStyle::BlinkingBar,
            );
        }

        self.scroller
            .render(outer_frame, buffer.text.lines().count(), self.focus);
    }
}
