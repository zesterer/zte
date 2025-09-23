use crate::{
    Args, Dir, Error,
    highlight::{Highlighter, Highlights},
    theme,
};
use clipboard::{ClipboardContext, ClipboardProvider};
use slotmap::{HopSlotMap, new_key_type};
use std::{
    collections::HashMap,
    io,
    ops::Range,
    path::{Path, PathBuf},
};

new_key_type! {
    pub struct BufferId;
    pub struct CursorId;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Cursor {
    pub base: usize,
    pub pos: usize,
    // Used to 'remember' the desired column when skipping over shorter lines
    desired_col: isize,
}

impl Cursor {
    fn reset_desired_col(&mut self, text: &Text) {
        self.desired_col = text.to_coord(self.pos)[0];
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        if self.base == self.pos {
            None
        } else {
            Some(self.base.min(self.pos)..self.base.max(self.pos))
        }
    }

    pub fn place_at(&mut self, pos: usize) {
        self.base = pos;
        self.pos = pos;
        // TODO: Reset desired position
    }

    pub fn select(&mut self, range: Range<usize>) {
        self.base = range.start;
        self.pos = range.end;
        // TODO: Reset desired position
    }
}

#[derive(Default)]
pub struct Text {
    chars: Vec<char>,
}

impl ToString for Text {
    fn to_string(&self) -> String {
        self.chars.iter().copied().collect()
    }
}

impl Text {
    // TODO: Remove this
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn to_coord(&self, pos: usize) -> [isize; 2] {
        let mut n = 0;
        let mut last_n = 0;
        let mut i: usize = 0;
        for line in self.lines() {
            last_n = n;
            i += 1;
            if (n..n + line.len()).contains(&pos) {
                break;
            }
            n += line.len();
        }
        [(pos - last_n) as isize, i.saturating_sub(1) as isize]
    }

    pub fn to_pos(&self, coord: [isize; 2]) -> usize {
        if coord[1] < 0 {
            return 0;
        }
        let mut pos = 0;
        for (i, line) in self.lines().enumerate() {
            if i as isize == coord[1] {
                return pos + coord[0].clamp(0, line.len().saturating_sub(1) as isize) as usize;
            } else {
                pos += line.len();
            }
        }
        pos.min(self.chars.len())
    }

    /// Return an iterator over the lines of the text.
    ///
    /// Guarantees:
    /// - If you sum the lengths of each line, it will be the same as the length (in characters) of the text
    pub fn lines(&self) -> impl Iterator<Item = &[char]> {
        let mut start = 0;
        let mut i = 0;
        let mut finished = false;
        core::iter::from_fn(move || {
            loop {
                let Some(c) = self.chars.get(i) else {
                    return if finished {
                        None
                    } else {
                        let line = &self.chars[start..];
                        finished = true;
                        Some(line)
                    };
                };
                i += 1;
                if *c == '\n' {
                    let line = &self.chars[start..i];
                    start = i;
                    return Some(line);
                }
            }
        })
    }

    fn indent_of_line(&self, line: isize) -> &[char] {
        let line_start = self.to_pos([0, line]);
        let mut i = 0;
        while self
            .chars()
            .get(line_start + i)
            .map_or(false, |c| [' ', '\t'].contains(c))
        {
            i += 1;
        }
        self.chars().get(line_start..line_start + i).unwrap_or(&[])
    }
}

#[derive(Default)]
pub struct Buffer {
    pub unsaved: bool,
    pub text: Text,
    pub highlights: Option<Highlights>,
    pub cursors: HopSlotMap<CursorId, Cursor>,
    pub dir: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub undo: Vec<Change>,
    pub redo: Vec<Change>,
    action_counter: usize,
    most_recent_rank: usize,
}

pub struct Change {
    kind: ChangeKind,
    action_id: usize,
    cursors: HashMap<CursorId, (Cursor, Cursor)>,
}

pub enum ChangeKind {
    Insert(usize, Vec<char>),
    Remove(usize, Vec<char>),
}

impl Change {
    fn invert(mut self) -> Self {
        self.kind = match self.kind {
            ChangeKind::Insert(at, s) => ChangeKind::Remove(at, s),
            ChangeKind::Remove(at, s) => ChangeKind::Insert(at, s),
        };
        for (from, to) in self.cursors.values_mut() {
            core::mem::swap(from, to);
        }
        self
    }
}

impl Buffer {
    pub fn from_file(path: PathBuf) -> Result<Self, Error> {
        let (unsaved, dir, chars, s) = match std::fs::read_to_string(&path) {
            Ok(s) => {
                let mut path = path.canonicalize()?;
                path.pop();
                (false, Some(path), s.chars().collect(), s)
            }
            // If the file doesn't exist, create a new file
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let dir = path
                    .parent()
                    .filter(|p| p.to_str() != Some(""))
                    .map(Path::to_owned)
                    .or_else(|| std::env::current_dir().ok());
                (true, dir, Vec::new(), String::new())
            }
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            unsaved,
            highlights: Highlighter::from_file_name(&path).map(|h| h.highlight(&chars)),
            text: Text { chars },
            cursors: HopSlotMap::default(),
            dir,
            path: Some(path),
            undo: Vec::new(),
            redo: Vec::new(),
            action_counter: 0,
            most_recent_rank: 0,
        })
    }

    pub fn save(&mut self) -> Result<(), Error> {
        if self.unsaved {
            std::fs::write(
                self.path.as_ref().expect("buffer must have path to save"),
                self.text.to_string(),
            )?;
            self.unsaved = false;
        }
        Ok(())
    }

    pub fn name(&self) -> Option<String> {
        Some(
            match self.path.as_ref()?.file_name().and_then(|n| n.to_str()) {
                Some(name) => format!("{}{name}", if self.unsaved { "* " } else { "" }),
                None => "<error>".to_string(),
            },
        )
    }

    fn update_highlights(&mut self) {
        self.highlights = self
            .highlights
            .take()
            .map(|hl| hl.highlighter.highlight(self.text.chars()));
    }

    pub fn reset(&mut self) {
        self.unsaved = true;

        self.text.chars.clear();
        self.update_highlights();
        // Reset cursors
        self.cursors.values_mut().for_each(|cursor| {
            *cursor = Cursor::default();
        });
        self.undo = Vec::new();
    }

    pub fn goto_cursor(&mut self, cursor_id: CursorId, coord: [isize; 2], set_base: bool) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        cursor.pos = self.text.to_pos(coord);
        cursor.reset_desired_col(&self.text);
        if set_base {
            cursor.base = cursor.pos;
        }
    }

    pub fn select_token_cursor(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        if let Some(tok) = self
            .highlights
            .as_ref()
            // Choose the longest token that the cursor is touching
            .and_then(|hl| {
                let a = hl.get_at(cursor.pos);
                let b = hl.get_at(cursor.pos.saturating_sub(1));
                a.zip(b)
                    .map(|(a, b)| {
                        if a.range.end - a.range.start > b.range.end - b.range.start {
                            a
                        } else {
                            b
                        }
                    })
                    .or(a)
                    .or(b)
            })
        {
            cursor.select(tok.range.clone());
        } else {
            // TODO: Bell
        }
    }

    pub fn select_all_cursor(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        cursor.base = 0;
        cursor.pos = self.text.chars().len();
    }

    fn indent_at(&mut self, mut pos: usize, forward: bool) {
        const TAB_ALIGN: usize = 4;

        if forward {
            let coord = self.text.to_coord(pos).map(|e| e.max(0) as usize);
            let next_up = |x: usize, n: usize| (x / n + 1) * n;
            let n = next_up(coord[0], TAB_ALIGN) - coord[0];
            self.insert(pos, (0..n).map(|_| ' '));
        } else {
            // First, find the next non-space character in the line
            while self.text.chars().get(pos) == Some(&' ') {
                pos += 1;
            }

            // Find the desired column, and hence the number of spaces to remove
            let coord = self.text.to_coord(pos).map(|e| e.max(0) as usize);
            let next_down = |x: usize, n: usize| (x.saturating_sub(1) / n) * n;
            let n = coord[0] - next_down(coord[0], TAB_ALIGN);

            // Keep removing whitespace until we hit the desired column
            for _ in 0..n {
                pos = match pos.checked_sub(1) {
                    Some(pos) if self.text.chars().get(pos) == Some(&' ') => {
                        self.remove(pos..pos + 1);
                        pos
                    }
                    _ => break,
                };
            }
        }
    }

    pub fn indent(&mut self, cursor_id: CursorId, forward: bool) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        if let Some(range) = cursor.selection() {
            let line_range = self.text.to_coord(range.start)[1]..=self.text.to_coord(range.end)[1];
            for line in line_range {
                self.indent_at(self.text.to_pos([0, line]), forward);
            }
        } else {
            let pos = cursor.pos;
            self.indent_at(pos, forward);
        }
    }

    pub fn move_cursor(
        &mut self,
        cursor_id: CursorId,
        dir: Dir,
        dist: [usize; 2],
        retain_base: bool,
        word: bool,
    ) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        match dir {
            Dir::Left => {
                cursor.pos = if !retain_base && cursor.base < cursor.pos {
                    cursor.base
                } else if let (true, Some(mut pos)) = (word, cursor.pos.checked_sub(1)) {
                    let mut class = self.text.chars().get(pos).copied().and_then(classify);
                    loop {
                        (class, pos) = if let Some(new_pos) = pos.checked_sub(1) {
                            let Some(new_class) =
                                self.text.chars().get(new_pos).copied().map(classify)
                            else {
                                break pos;
                            };
                            if (class.is_some() && new_class.is_none())
                                || matches!((class, new_class), (Some(c), Some(n)) if c != n)
                            {
                                break pos;
                            } else {
                                (new_class, new_pos)
                            }
                        } else {
                            break pos;
                        };
                    }
                } else {
                    cursor.pos.saturating_sub(dist[0])
                };
                cursor.reset_desired_col(&self.text);
            }
            Dir::Right => {
                cursor.pos = if !retain_base && cursor.base > cursor.pos {
                    cursor.base
                } else if word {
                    let mut pos = cursor.pos;
                    let mut class = self.text.chars().get(pos).copied().and_then(classify);
                    loop {
                        let Some(new_class) = self.text.chars().get(pos).copied().map(classify)
                        else {
                            break pos;
                        };
                        (class, pos) = if (class.is_some() && new_class.is_none())
                            || matches!((class, new_class), (Some(c), Some(n)) if c != n)
                        {
                            break pos;
                        } else {
                            (new_class, pos + 1)
                        };
                    }
                } else {
                    (cursor.pos + dist[0]).min(self.text.chars.len())
                };
                cursor.reset_desired_col(&self.text);
            }
            Dir::Up => {
                let coord = self.text.to_coord(cursor.pos);
                // Special case: pressing 'up' at the top of the screen resets the cursor to the beginning
                if coord[1] <= 0 {
                    cursor.pos = 0;
                    cursor.reset_desired_col(&self.text);
                } else {
                    cursor.pos = self
                        .text
                        .to_pos([cursor.desired_col, coord[1] - dist[1] as isize]);
                }
            }
            Dir::Down => {
                let coord = self.text.to_coord(cursor.pos);
                cursor.pos = self
                    .text
                    .to_pos([cursor.desired_col, coord[1] + dist[1] as isize]);
            }
        };

        if !retain_base {
            cursor.base = cursor.pos;
        }
    }

    pub fn begin_action(&mut self) {
        self.action_counter += 1;
    }

    fn push_undo(&mut self, mut change: Change) {
        self.redo.clear(); // TODO: Maybe add tree undos?

        let Some(last) = self.undo.last_mut() else {
            return self.undo.push(change);
        };

        // Attempt to merge changes together
        match (&mut last.kind, &mut change.kind) {
            (ChangeKind::Insert(at, s), ChangeKind::Insert(at2, s2)) if *at + s.len() == *at2 => {
                s.append(s2);
            }
            (ChangeKind::Remove(at, s), ChangeKind::Remove(at2, s2)) if *at == *at2 + s2.len() => {
                s2.append(s);
                *s = core::mem::take(s2);
                *at = *at2;
            }
            _ => return self.undo.push(change),
        }

        last.action_id = change.action_id;

        for (id, (from2, to2)) in change.cursors {
            last.cursors
                .entry(id)
                .and_modify(|(_, to)| *to = to2)
                .or_insert((from2, to2));
        }
    }

    fn apply_change(&mut self, change: &Change) {
        match &change.kind {
            ChangeKind::Insert(at, s) => {
                for (i, c) in s.iter().enumerate() {
                    self.text.chars.insert(at + i, *c);
                }
            }
            ChangeKind::Remove(at, s) => {
                self.text.chars.drain(*at..*at + s.len());
            }
        }
        for (id, (_, to)) in change.cursors.iter() {
            if let Some(c) = self.cursors.get_mut(*id) {
                // panic!("Changing {c:?} to {to:?}");
                *c = *to;
            }
        }
        self.update_highlights();
    }

    fn undo_or_redo(&mut self, is_undo: bool) -> bool {
        if let Some(mut change) = if is_undo {
            self.undo.pop()
        } else {
            self.redo.pop()
        } {
            let action_id = change.action_id;
            // Keep applying previous changes provided they were part of the same action
            loop {
                let inv_change = change.invert();
                self.apply_change(&inv_change);
                if is_undo {
                    self.redo.push(inv_change)
                } else {
                    self.undo.push(inv_change)
                }
                change = if let Some(c) = (if is_undo {
                    &mut self.undo
                } else {
                    &mut self.redo
                })
                .pop_if(|c| c.action_id == action_id)
                {
                    c
                } else {
                    break true;
                };
            }
        } else {
            false
        }
    }

    pub fn undo(&mut self) -> bool {
        self.undo_or_redo(true)
    }

    pub fn redo(&mut self) -> bool {
        self.undo_or_redo(false)
    }

    fn insert_inner(&mut self, pos: usize, chars: impl IntoIterator<Item = char>) -> Change {
        let chars = chars.into_iter().collect::<Vec<_>>();
        let mut n = 0;
        let base = pos.min(self.text.chars.len());
        for c in &chars {
            self.text.chars.insert(base + n, *c);
            n += 1;
        }
        self.update_highlights();
        Change {
            kind: ChangeKind::Insert(base, chars),
            action_id: self.action_counter,
            cursors: self
                .cursors
                .iter_mut()
                .map(|(id, cursor)| {
                    let old = *cursor;
                    if cursor.base >= pos {
                        cursor.base += n;
                    }
                    if cursor.pos >= pos {
                        cursor.pos += n;
                        cursor.reset_desired_col(&self.text);
                    }
                    (id, (old, *cursor))
                })
                .collect(),
        }
    }

    pub fn insert(&mut self, pos: usize, chars: impl IntoIterator<Item = char>) {
        self.unsaved = true;
        let change = self.insert_inner(pos, chars);
        self.push_undo(change);
    }

    // Assumes range is well-formed
    fn remove_inner(&mut self, range: Range<usize>) -> Change {
        self.unsaved = true;

        // TODO: Bell if false?
        let removed = self.text.chars.drain(range.clone()).collect();
        self.update_highlights();
        Change {
            kind: ChangeKind::Remove(range.start, removed),
            action_id: self.action_counter,
            cursors: self
                .cursors
                .iter_mut()
                .map(|(id, cursor)| {
                    let old = *cursor;
                    if cursor.base >= range.start {
                        cursor.base = cursor
                            .base
                            .saturating_sub(range.end - range.start)
                            .max(range.start);
                    }
                    if cursor.pos >= range.start {
                        cursor.pos = cursor
                            .pos
                            .saturating_sub(range.end - range.start)
                            .max(range.start);
                        cursor.reset_desired_col(&self.text);
                    }
                    (id, (old, *cursor))
                })
                .collect(),
        }
    }

    // Assumes range is well-formed
    pub fn remove(&mut self, range: Range<usize>) {
        self.unsaved = true;
        let change = self.remove_inner(range);
        self.push_undo(change);
    }

    pub fn insert_after(&mut self, cursor_id: CursorId, chars: impl IntoIterator<Item = char>) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        let old_cursor = *cursor;
        self.insert(old_cursor.pos, chars);
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        *cursor = old_cursor;
    }

    pub fn enter(&mut self, cursor_id: CursorId, chars: impl IntoIterator<Item = char>) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
            self.enter(cursor_id, chars);
        } else {
            self.insert(cursor.pos, chars);
        }
    }

    pub fn backspace(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        let line_start = self.text.to_pos([0, self.text.to_coord(cursor.pos)[1]]);
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else
        /*if line_start != cursor.pos && (line_start..cursor.pos)
            .all(|p| self.text.chars().get(p).map_or(false, |c| [' ', '\t'].contains(c)))
        {
            self.remove(line_start..cursor.pos);
            self.backspace(cursor_id); // Remove the newline too
        } else*/
        if let Some(pos) = cursor.pos.checked_sub(1) {
            // If a backspace is performed on a space, a deindent takes place instead
            if self.text.chars().get(pos) == Some(&' ') {
                self.indent_at(pos, false);
            } else {
                self.remove(pos..pos + 1);
            }
        }
    }

    pub fn delete(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else {
            self.remove(cursor.pos..cursor.pos + 1);
        }
    }

    pub fn newline(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        let coord = self.text.to_coord(cursor.pos);
        let line_start = self.text.to_pos([0, coord[1]]);

        let prev_indent = self
            .text
            .indent_of_line(coord[1])
            .iter()
            .copied()
            .take(coord[0] as usize)
            .collect::<Vec<_>>();
        let next_indent = self.text.indent_of_line(coord[1] + 1).to_vec();

        let (close_block, extra_indent, trailing_indent, base_indent) = if let Some(last_pos) =
            cursor
                .selection()
                .map_or(cursor.pos, |s| s.start)
                .checked_sub(1)
            && let Some(last_char) = self.text.chars().get(last_pos)
            && let Some((l, r)) = [('(', ')'), ('[', ']'), ('{', '}')]
                .iter()
                .find(|(l, _)| l == last_char)
            && let next_pos = cursor.selection().map_or(cursor.pos, |s| s.end)
            && let next_tok = self
                .text
                .chars()
                .get(next_pos..)
                .unwrap_or(&[])
                .iter()
                .filter(|c| !c.is_ascii_whitespace())
                .next()
            && let next_char = self.text.chars().get(next_pos)
        {
            let close_block = (next_tok != Some(r)
                && next_indent
                    .strip_prefix(&*prev_indent)
                    .map_or(false, |i| i.is_empty()))
                || (next_char != Some(r)
                    && prev_indent
                        .strip_prefix(&*next_indent)
                        .map_or(false, |i| !i.is_empty()));
            (
                if close_block { Some(*r) } else { None },
                true,
                close_block || self.text.chars().get(next_pos) == Some(r),
                prev_indent,
            )
        } else {
            (None, false, false, prev_indent)
        };

        // Indent to same level as last line
        self.enter(
            cursor_id,
            ['\n'].into_iter().chain(base_indent.iter().copied()),
        );

        if let Some(r) = close_block {
            self.insert_after(cursor_id, [r]);
        }
        if trailing_indent {
            self.insert_after(cursor_id, core::iter::once('\n').chain(base_indent));
        }
        if extra_indent {
            self.indent(cursor_id, true);
        }
    }

    pub fn copy(&mut self, cursor_id: CursorId) -> bool {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return false;
        };
        if let Some(text) = cursor.selection().and_then(|s| self.text.chars().get(s))
            && ClipboardContext::new()
                .and_then(|mut ctx| ctx.set_contents(text.iter().copied().collect()))
                .is_ok()
        {
            true
        } else {
            false
        }
    }

    pub fn cut(&mut self, cursor_id: CursorId) -> bool {
        if self.copy(cursor_id) {
            self.backspace(cursor_id);
            true
        } else {
            false
        }
    }

    pub fn paste(&mut self, cursor_id: CursorId) -> bool {
        if let Ok(s) = ClipboardContext::new().and_then(|mut ctx| ctx.get_contents()) {
            self.enter(cursor_id, s.chars());
            true
        } else {
            false
        }
    }

    pub fn duplicate(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        if let Some(s) = cursor.selection()
            && let Some(text) = cursor.selection().and_then(|s| self.text.chars().get(s))
        {
            // cursor.place_at(s.end);
            self.insert_after(cursor_id, text.to_vec())
        } else {
            let coord = self.text.to_coord(cursor.pos);
            let line = self
                .text
                .lines()
                .nth(coord[1].max(0) as usize)
                .map(|l| l.to_vec());
            if let Some(line) = line {
                let end_of_line = self.text.to_pos([0, coord[1] + 1]);
                self.insert(end_of_line, line);
            }
        }
    }

    pub fn comment(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        let lines = cursor
            .selection()
            .map(|s| self.text.to_coord(s.start)[1]..=self.text.to_coord(s.end)[1])
            .unwrap_or_else(|| {
                let coord = self.text.to_coord(cursor.pos);
                coord[1]..=coord[1]
            });
        let mut indent: Option<&[char]> = None;
        for line_idx in lines.clone() {
            indent = Some(match (indent, self.text.indent_of_line(line_idx)) {
                (Some(indent), new_indent) => {
                    &new_indent[..indent
                        .iter()
                        .zip(new_indent)
                        .take_while(|(x, y)| x == y)
                        .count()]
                }
                (None, new_indent) => new_indent,
            });
        }
        let indent = indent.unwrap_or(&[]).to_vec();
        for line_idx in lines {
            let pos = self.text.to_pos([indent.len() as isize, line_idx]);
            if self
                .text
                .chars()
                .get(pos..)
                .map_or(false, |l| l.starts_with(&['/', '/', ' ']))
            {
                self.remove(pos..pos + 3);
            } else {
                self.insert(pos, "// ".chars());
            }
        }
    }

    pub fn start_session(&mut self) -> CursorId {
        self.cursors.insert(Cursor::default())
    }

    pub fn end_session(&mut self, cursor_id: CursorId) {
        self.cursors.remove(cursor_id);
    }
}

// CLassify the character by property
fn classify(c: char) -> Option<u8> {
    match c {
        ' ' | '\t' => None,
        '\n' => Some(0),
        c if c.is_alphanumeric() || c == '_' => Some(1),
        _ => Some(2),
    }
}

pub struct State {
    pub buffers: HopSlotMap<BufferId, Buffer>,
    pub tick: u64,
    pub theme: theme::Theme,
    pub most_recent_counter: usize,
}

impl TryFrom<Args> for State {
    type Error = Error;
    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let mut this = Self {
            buffers: HopSlotMap::default(),
            tick: 0,
            theme: theme::Theme::default(),
            most_recent_counter: 0,
        };

        if args.paths.is_empty() {
            this.buffers.insert(Buffer::default());
        } else {
            for path in args.paths {
                this.buffers.insert(Buffer::from_file(path)?);
            }
        }

        Ok(this)
    }
}

impl State {
    pub fn open_or_get(&mut self, path: PathBuf) -> Result<BufferId, Error> {
        let true_path = path.canonicalize()?;
        if let Some((buffer_id, _)) = self.buffers.iter().find(|(_, b)| {
            b.path.as_ref().and_then(|p| p.canonicalize().ok()).as_ref() == Some(&true_path)
        }) {
            Ok(buffer_id)
        } else {
            Ok(self.buffers.insert(Buffer::from_file(path)?))
        }
    }

    pub fn tick(&mut self) {
        self.tick += 1;
    }

    pub fn set_most_recent(&mut self, buffer: BufferId) {
        if let Some(buffer) = self.buffers.get_mut(buffer) {
            self.most_recent_counter += 1;
            buffer.most_recent_rank = self.most_recent_counter;
        }
    }

    pub fn most_recent(&self) -> Vec<BufferId> {
        let mut most_recent = self.buffers.keys().collect::<Vec<_>>();
        most_recent.sort_by_key(|b| core::cmp::Reverse(self.buffers[*b].most_recent_rank));
        most_recent
    }
}
