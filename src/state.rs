use crate::{
    Args, Dir, Error,
    highlight::{Highlights, Token},
    lang::LangPack,
    theme,
    ui::{Term, TermWindow},
};
#[cfg(feature = "clipboard")]
use clipboard::{ClipboardContext, ClipboardProvider};
use crop::{Rope, RopeSlice};
use slotmap::{DenseSlotMap, new_key_type};
use std::{
    collections::HashMap,
    io,
    ops::{Range, RangeBounds},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

new_key_type! {
    pub struct BufferId;
    pub struct CursorId;
    pub struct TermId;
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
    // chars: Vec<char>,
    inner: Rope,
}

impl ToString for Text {
    fn to_string(&self) -> String {
        self.inner.chunks().collect()
    }
}

trait RopeExt {
    fn starts_with(&self, cs: impl IntoIterator<Item = char>) -> bool;
}

impl RopeExt for RopeSlice<'_> {
    fn starts_with(&self, cs: impl IntoIterator<Item = char>) -> bool {
        cs.into_iter()
            .zip(self.chars().map(Some).chain(core::iter::repeat(None)))
            .all(|(c, this)| Some(c) == this)
    }
}

impl Text {
    pub fn new(s: &str) -> Self {
        Self {
            // chars: s.chars().collect(),
            inner: Rope::from(s),
        }
    }

    pub fn clear(&mut self) {
        self.inner = Rope::new();
    }

    pub fn len(&self) -> usize {
        self.inner.byte_len()
    }

    fn slice_char_indices(slice: RopeSlice<'_>) -> impl Iterator<Item = (char, usize)> {
        slice.chars().scan(0, |s, c| {
            let i = *s;
            *s += c.len_utf8();
            Some((c, i))
        })
    }

    pub fn char_indices(&self) -> impl Iterator<Item = (char, usize)> {
        Self::slice_char_indices(self.slice(..))
    }

    #[track_caller]
    pub fn slice(&self, byte_range: impl RangeBounds<usize>) -> RopeSlice<'_> {
        self.inner.byte_slice(byte_range)
    }

    pub fn to_coord(&self, pos: usize) -> [isize; 2] {
        if self.inner.line_len() == 0 {
            [0, 0]
        } else {
            let y = self.inner.line_of_byte(pos.min(self.len()));
            if y >= self.inner.line_len() {
                [0, self.inner.line_len() as isize]
            } else {
                let line_byte = pos.saturating_sub(self.inner.byte_of_line(y));
                [
                    Self::slice_char_indices(self.inner.line(y))
                        .take_while(|(_, i)| *i < line_byte)
                        .count() as isize,
                    y as isize,
                ]
            }
        }
    }

    pub fn to_pos(&self, coord: [isize; 2]) -> usize {
        if coord[1] < 0 {
            0
        } else if coord[1] as usize >= self.inner.line_len() {
            self.inner.byte_len()
        } else {
            let line = self.inner.line(coord[1] as usize);
            self.inner.byte_of_line(coord[1] as usize)
                + line
                    .chars()
                    .map(|c| c.len_utf8())
                    .enumerate()
                    .take_while(|(i, _)| *i < coord[0].max(0) as usize)
                    .map(|(_, n)| n)
                    .sum::<usize>()
        }
    }

    /// Return an iterator over the lines of the text.
    ///
    /// Guarantees:
    /// - If you sum the lengths of each line, it will be the same as the length (in characters) of the text
    pub fn lines(&self) -> impl ExactSizeIterator<Item = RopeSlice<'_>> {
        self.inner.raw_lines()
    }
    pub fn line(&self, line: usize) -> Option<RopeSlice<'_>> {
        if line >= self.inner.line_len() {
            None
        } else {
            Some(self.inner.line_slice(line..line + 1))
        }
    }

    pub fn indent_of_line(&self, line: isize) -> RopeSlice<'_> {
        let Some(line) = self.line(line.max(0) as usize) else {
            return self.slice(0..0);
        };
        if let Some((_, i)) = Text::slice_char_indices(line).find(|(c, _)| ![' ', '\t'].contains(c))
        {
            line.byte_slice(..i)
        } else {
            line
        }
    }

    fn start_of_line_text(&self, line: isize) -> Result<usize, usize> {
        let start = self.to_pos([0, line]) + self.indent_of_line(line).byte_len();
        // TODO: Detect \r\n too
        if self.slice(start..).chars().next() == Some('\n') {
            Err(start)
        } else {
            Ok(start)
        }
    }

    fn start_of_line(&self, line: isize) -> RopeSlice<'_> {
        self.start_of_line_text(line)
            .map(|i| {
                if line + 1 >= self.inner.line_len() as isize {
                    self.slice(i..)
                } else {
                    self.slice(i..self.inner.byte_of_line(line as usize + 1))
                }
            })
            .unwrap_or_else(|_| self.slice(0..0))
    }

    fn line_range(&self, line: isize) -> Range<usize> {
        let start = self.to_pos([0, line]);
        let end = self.to_pos([100000000, line]);
        start..end + 1
    }
}

struct OnDisk {
    path: PathBuf,
    last_observed_modification: SystemTime,
    pub unsaved: bool,
    diverged: bool,
}

#[derive(Default)]
pub struct Buffer {
    pub text: Text,
    pub lang: LangPack,
    pub cursors: DenseSlotMap<CursorId, Cursor>,
    on_disk: Option<OnDisk>,
    pub undo: Vec<Change>,
    pub redo: Vec<Change>,
    undo_dont_merge: bool,
    action_counter: usize,
    last_switch: Option<SystemTime>,

    // Note: ensure `sync_highlights` is called before use
    pub highlights: Highlights,
    highlights_stale: bool,
}

pub struct Change {
    kind: ChangeKind,
    action_id: usize,
    cursors: HashMap<CursorId, (Cursor, Cursor)>,
}

pub enum ChangeKind {
    Insert(usize, String),
    Remove(usize, String),
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
    pub fn anonymous() -> Self {
        Self::default()
    }

    pub fn file(unsaved: bool, content: &str, path: PathBuf) -> Self {
        Self {
            highlights: Highlights::default(),
            highlights_stale: true,
            lang: LangPack::from_file_name(&path),
            text: Text::new(content),
            cursors: DenseSlotMap::default(),
            on_disk: Some(OnDisk {
                last_observed_modification: std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or_else(|_| SystemTime::now()),
                path,
                unsaved,
                diverged: false,
            }),
            undo: Vec::new(),
            redo: Vec::new(),
            undo_dont_merge: false,
            action_counter: 0,
            last_switch: Some(SystemTime::now()),
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.on_disk.as_ref().map(|on_disk| &on_disk.path)
    }

    pub fn open(path: PathBuf) -> Result<Self, Error> {
        let path = path.canonicalize()?;
        let (unsaved, s) = match std::fs::read_to_string(&path) {
            Ok(s) => (false, s),
            Err(err) => return Err(err.into()),
        };
        Ok(Self::file(unsaved, &s, path))
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), Error> {
        // Ensure trailing newline exists
        if self
            .text
            .slice(..)
            .chars()
            .nth_back(0)
            .map_or(false, |c| c != '\n')
        {
            self.insert(self.text.len(), ['\n']);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.text.to_string())?;
        self.on_disk = Some(OnDisk {
            last_observed_modification: std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| SystemTime::now()),
            path,
            diverged: false,
            unsaved: false,
        });
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        if let Some(on_disk) = self.on_disk.take() {
            self.save_as(on_disk.path)
        } else {
            // TODO: Not okay!
            Ok(())
        }
    }

    pub fn move_to(&mut self, path: PathBuf) -> Result<(), Error> {
        if let Some(old_on_disk) = self.on_disk.take() {
            // First, try renaming the old file
            if std::fs::rename(&old_on_disk.path, &path).is_ok() {
                self.on_disk = Some(OnDisk {
                    last_observed_modification: std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or_else(|_| SystemTime::now()),
                    path,
                    diverged: false,
                    unsaved: false,
                });
                Ok(())
            } else {
                // If that didn't work, save it in the new location and remove the old one
                self.save_as(path)?;
                std::fs::remove_file(old_on_disk.path)?;
                Ok(())
            }
        } else {
            Err(Error::FileNotOnDisk)
        }
    }

    pub fn name(&self) -> Option<String> {
        if let Some(on_disk) = &self.on_disk
            && let Some(name) = on_disk.path.file_name().and_then(|n| n.to_str())
        {
            Some(format!(
                "{}{name}",
                if on_disk.diverged {
                    "! "
                } else if on_disk.unsaved {
                    "* "
                } else {
                    ""
                }
            ))
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        assert!(self.on_disk.is_none());

        self.text.clear();
        self.highlights_stale = true;
        self.highlights.damage_all();
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

    pub fn sync_highlights(&mut self) {
        // Update highlights, if necessary
        if self.highlights_stale {
            self.highlights.sync(&self.lang, &self.text);
            self.highlights_stale = false;
        }
    }

    pub fn token_at_pos(&mut self, pos: usize) -> Option<&Token> {
        self.sync_highlights();
        self.highlights
            .get_at(&self.lang.highlighter, &self.text, pos)
    }

    pub fn token_at_coord(&mut self, coord: [isize; 2]) -> Option<&Token> {
        self.token_at_pos(self.text.to_pos(coord))
    }

    pub fn select_cursor(&mut self, cursor_id: CursorId, range: Range<usize>) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        cursor.select(range);
    }

    pub fn select_word_cursor(&mut self, cursor_id: CursorId) -> bool {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return false;
        };

        if let Some(class) = self
            .text
            .slice(cursor.pos..)
            .chars()
            .next()
            .and_then(classify)
            // If there's no token under the cursor, try looking left
            .or_else(|| {
                self.text
                    .slice(cursor.pos..)
                    .chars()
                    .next_back()
                    .and_then(classify)
            })
        {
            let start = cursor.pos.saturating_sub(
                self.text
                    .slice(..cursor.pos)
                    .chars()
                    .rev()
                    .take_while(|c| classify(*c) == Some(class))
                    .map(|c| c.len_utf8())
                    .sum(),
            );
            let end = cursor.pos
                + self
                    .text
                    .slice(cursor.pos..)
                    .chars()
                    .take_while(|c| classify(*c) == Some(class))
                    .map(|c| c.len_utf8())
                    .sum::<usize>();
            cursor.select(start..end);
            true
        } else {
            false
        }
    }

    pub fn select_block_cursor(&mut self, cursor_id: CursorId) -> bool {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return false;
        };

        if let Some((_, span)) = self.highlights.get_delim_at(|s| {
            (s.start..=s.end).contains(&cursor.pos)
                && Some(s.clone()) != cursor.selection()
                && self.text.to_coord(s.start)[1] != self.text.to_coord(s.end)[1]
        }) {
            cursor.select(span.end..span.start);
            cursor.reset_desired_col(&self.text);
            true
        } else {
            false
        }
    }

    pub fn select_all_cursor(&mut self, cursor_id: CursorId) {
        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        cursor.base = 0;
        cursor.pos = self.text.len();
    }

    fn indent_at(&mut self, mut pos: usize, forward: bool) {
        const TAB_ALIGN: usize = 4;

        if forward {
            let coord = self.text.to_coord(pos).map(|e| e.max(0) as usize);
            let next_up = |x: usize, n: usize| (x / n + 1) * n;
            let n = next_up(coord[0], TAB_ALIGN) - coord[0];
            self.insert(pos, (0..n).map(|_| ' '));
        } else {
            // Find the desired column, and hence the number of spaces to remove
            let coord = self.text.to_coord(pos).map(|e| e.max(0) as usize);
            let next_down = |x: usize, n: usize| (x.saturating_sub(1) / n) * n;
            let n = coord[0] - next_down(coord[0], TAB_ALIGN);

            // Keep removing whitespace until we hit the desired column
            for _ in 0..n {
                if let Some(c) = self.text.slice(..pos).chars().next_back()
                    && c == ' '
                {
                    pos -= c.len_utf8();
                    self.remove(pos..pos + c.len_utf8());
                    pos
                } else {
                    break;
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
                // For maximum flexibility, indent/deindent from the end of the indentation
                self.indent_at(
                    self.text.to_pos([0, line]) + self.text.indent_of_line(line).byte_len(),
                    forward,
                );
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
        self.undo_checkpoint();
        self.move_cursor_inner(cursor_id, dir, dist, retain_base, word);
    }

    pub fn move_cursor_inner(
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
                let checked_prev = |pos: usize| {
                    self.text
                        .slice(..pos)
                        .chars()
                        .nth_back(0)
                        .map(|c| pos.saturating_sub(c.len_utf8()))
                };
                cursor.pos = if !retain_base && cursor.base < cursor.pos {
                    cursor.base
                } else if !retain_base && cursor.base > cursor.pos {
                    cursor.pos
                } else if let (true, Some(mut pos)) = (word, checked_prev(cursor.pos)) {
                    let mut class = self.text.slice(pos..).chars().next().and_then(classify);
                    loop {
                        (class, pos) = if let Some(new_pos) = checked_prev(pos) {
                            let Some(new_class) =
                                self.text.slice(new_pos..).chars().next().map(classify)
                            else {
                                break pos;
                            };
                            if class != new_class {
                                break pos;
                            } else {
                                (new_class, new_pos)
                            }
                        } else {
                            break pos;
                        };
                    }
                } else {
                    checked_prev(cursor.pos).unwrap_or(0)
                };
                cursor.reset_desired_col(&self.text);
            }
            Dir::Right => {
                cursor.pos = if !retain_base && cursor.base > cursor.pos {
                    cursor.base
                } else if !retain_base && cursor.base < cursor.pos {
                    cursor.pos
                } else if word {
                    let mut pos = cursor.pos;
                    let mut class = self.text.slice(pos..).chars().next().and_then(classify);
                    loop {
                        let Some(c) = self.text.slice(pos..).chars().next() else {
                            break pos;
                        };
                        let new_class = classify(c);
                        (class, pos) = if class != new_class {
                            break pos;
                        } else {
                            (new_class, pos + c.len_utf8())
                        };
                    }
                } else {
                    let bytes = self
                        .text
                        .slice(cursor.pos..)
                        .chars()
                        .next()
                        .map_or(0, |c| c.len_utf8());
                    (cursor.pos + bytes).min(self.text.len())
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

        if self.undo_dont_merge {
            self.undo_dont_merge = false;
            return self.undo.push(change);
        }

        // Attempt to merge changes together
        match (&mut last.kind, &mut change.kind) {
            (ChangeKind::Insert(at, s), ChangeKind::Insert(at2, s2)) if *at + s.len() == *at2 => {
                *s += s2;
            }
            (ChangeKind::Remove(at, s), ChangeKind::Remove(at2, s2)) if *at == *at2 + s2.len() => {
                *s2 += s;
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
                self.text.inner.insert(*at, &s);
                self.highlights.damage_insert(*at..*at + s.len());
            }
            ChangeKind::Remove(at, s) => {
                self.text.inner.delete(*at..*at + s.len());
                self.highlights.damage_remove(*at..*at + s.len());
            }
        }
        for (id, (_, to)) in change.cursors.iter() {
            if let Some(c) = self.cursors.get_mut(*id) {
                *c = *to;
            }
        }
        self.highlights_stale = true;
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

    pub fn undo_checkpoint(&mut self) {
        self.undo_dont_merge = true;
    }

    fn insert_inner(&mut self, pos: usize, chars: impl IntoIterator<Item = char>) -> Change {
        if let Some(on_disk) = &mut self.on_disk {
            on_disk.unsaved = true;
        }

        // TODO: Don't allocate
        let s = chars.into_iter().collect::<String>();
        let base = pos.min(self.text.len());
        self.text.inner.insert(base, &s);
        self.highlights_stale = true;
        self.highlights.damage_insert(base..base + s.len());
        Change {
            action_id: self.action_counter,
            cursors: self
                .cursors
                .iter_mut()
                .map(|(id, cursor)| {
                    let old = *cursor;
                    if cursor.base >= pos {
                        cursor.base += s.len();
                    }
                    if cursor.pos >= pos {
                        cursor.pos += s.len();
                        cursor.reset_desired_col(&self.text);
                    }
                    (id, (old, *cursor))
                })
                .collect(),
            kind: ChangeKind::Insert(base, s),
        }
    }

    pub fn insert(&mut self, pos: usize, chars: impl IntoIterator<Item = char>) {
        let change = self.insert_inner(pos, chars);
        self.push_undo(change);
    }

    // Assumes range is well-formed
    fn remove_inner(&mut self, range: Range<usize>) -> Change {
        if let Some(on_disk) = &mut self.on_disk {
            on_disk.unsaved = true;
        }

        // Force range to be valid
        let range = range.start.min(self.text.len())..range.end.min(self.text.len());

        let removed = self.text.slice(range.clone()).to_string();
        self.text.inner.delete(range.clone());
        self.highlights_stale = true;
        self.highlights.damage_remove(range.clone());
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
        let change = self.remove_inner(range);
        self.push_undo(change);
    }

    // Assumes range is well-formed
    pub fn replace(&mut self, range: Range<usize>, chars: impl IntoIterator<Item = char>) {
        self.remove(range.clone());
        self.insert(range.start, chars);
    }

    pub fn insert_after(
        &mut self,
        cursor_id: CursorId,
        at: Option<usize>,
        chars: impl IntoIterator<Item = char>,
    ) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        let old_cursor = *cursor;
        self.insert(at.unwrap_or(old_cursor.pos), chars);
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

        if let Some(reflow_col) = self.lang.reflow_col {
            let Some(cursor) = self.cursors.get(cursor_id) else {
                return;
            };
            let old_cursor_pos = cursor.pos;
            let mut line_idx = self.text.to_coord(cursor.pos)[1].max(0) as usize;

            loop {
                let Some(line) = self.text.line(line_idx) else {
                    break;
                };
                // Find an appropriate place to split the line
                if let Some((reflow_col, _)) = line
                    .chars()
                    .take(reflow_col)
                    .enumerate()
                    // TODO: No
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .find(|(_, c)| *c == ' ')
                    && reflow_col > 0
                {
                    let line_start = self.text.to_pos([0, line_idx as isize]);
                    let next_line_char = self
                        .text
                        .lines()
                        .nth(line_idx + 1)
                        .and_then(|l| l.chars().nth(0));
                    if let Some(newline) = line.chars().next_back()
                        && newline == '\n'
                        && let Some(next_line_char) = next_line_char
                        && !next_line_char.is_whitespace()
                    {
                        self.replace(
                            line_start + line.byte_len().saturating_sub(newline.len_utf8())
                                ..line_start + line.byte_len(),
                            [' '],
                        );
                    }
                    let newline = '\n';
                    self.replace(
                        line_start + reflow_col..line_start + reflow_col + newline.len_utf8(),
                        [newline],
                    );
                    let Some(cursor) = self.cursors.get_mut(cursor_id) else {
                        return;
                    };
                    cursor.place_at(old_cursor_pos);
                    line_idx += 1;
                } else {
                    break;
                }
            }
        }
    }

    pub fn backspace(&mut self, cursor_id: CursorId, word: bool) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        let coord = self.text.to_coord(cursor.pos);
        let line_start = self.text.to_pos([0, coord[1]]);
        // At start of line, remove entire line
        let line_text_start = self.text.start_of_line_text(coord[1]);

        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else if !word
            && cursor.pos != line_start
            && cursor.pos == line_text_start.unwrap_or_else(|s| s)
        {
            // If a simple backspace is performed at the start of indentation, a deindent takes place instead
            // Ensure there's only whitespace to our left
            self.indent_at(cursor.pos, false);
        } else {
            self.move_cursor_inner(cursor_id, Dir::Left, [1, 1], true, word);
            let Some(cursor) = self.cursors.get(cursor_id) else {
                return;
            };
            let Some(sel) = cursor.selection() else {
                return;
            };
            self.remove(sel);
        }
    }

    pub fn delete(&mut self, cursor_id: CursorId, word: bool) {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };
        if let Some(selection) = cursor.selection() {
            self.remove(selection);
        } else {
            self.move_cursor_inner(cursor_id, Dir::Right, [1, 1], true, word);
            let Some(cursor) = self.cursors.get(cursor_id) else {
                return;
            };
            let Some(sel) = cursor.selection() else {
                return;
            };
            self.remove(sel);
        }
    }

    pub fn newline(&mut self, cursor_id: CursorId) {
        self.undo_checkpoint();

        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };

        let coord = self.text.to_coord(cursor.pos);
        let next_line_start = self.text.to_pos([0, coord[1] + 1]);

        let prev_indent = self
            .text
            .indent_of_line(coord[1])
            .chars()
            .take(coord[0] as usize)
            .collect::<Vec<_>>();
        let next_indent = self
            .text
            .indent_of_line(coord[1] + 1)
            .chars()
            .collect::<Vec<_>>();

        // Determine whether we're creating/forming a new code block
        let (close_block, end_of_block, base_indent) = if let Some(last_pos) = cursor
            .selection()
            .map_or(cursor.pos, |s| s.start)
            .checked_sub(1)
            && let Some(last_char) = self.text.slice(last_pos..).chars().next()
            && let Some((_, r)) = self.lang.delims.iter().find(|(l, _)| *l == last_char)
        {
            let (end_of_block, end_needs_indent) = self
                .highlights
                .get_delim_at(|r| (r.start + 1..=r.end).contains(&cursor.pos))
                .filter(|(_, r)| r.start + 1 == cursor.pos)
                .map(|(_, r)| r.end.saturating_sub(1))
                .filter(|end| self.text.to_coord(*end)[1] == coord[1])
                .map(|end| (end, true))
                .unwrap_or((next_line_start.saturating_sub(1), true));
            let needs_closing = self.text.slice(end_of_block..).chars().next() != Some(*r);
            let creating_block = false
                // Case 1: A block is being created from an existing inline one
                || (!needs_closing && self.text.to_coord(end_of_block)[1] == coord[1])
                || next_indent
                    .strip_prefix(&*prev_indent)
                    .map_or(false, |i| i.is_empty())
                || (needs_closing
                    && prev_indent
                        .strip_prefix(&*next_indent)
                        .map_or(false, |i| !i.is_empty()));
            (
                (creating_block && needs_closing).then_some(*r),
                creating_block.then_some((end_of_block, end_needs_indent)),
                if prev_indent.len() < next_indent.len() && !creating_block {
                    next_indent
                } else {
                    prev_indent
                },
            )
        } else {
            (None, None, prev_indent)
        };

        // Where is the end of the new code block?
        if let Some((end_of_block, end_needs_indent)) = end_of_block {
            if let Some(r) = close_block {
                self.insert_after(cursor_id, Some(end_of_block), [r]);
            }

            // Indent the block closer to the base level
            if end_needs_indent {
                self.insert_after(
                    cursor_id,
                    Some(end_of_block),
                    core::iter::once('\n').chain(base_indent.iter().copied()),
                );
            }
        }

        // Indent to same level as last line
        self.enter(cursor_id, ['\n'].into_iter().chain(base_indent));
        if end_of_block.is_some() {
            // If we're starting a new block, increase the indent
            self.indent(cursor_id, true);
        }
    }

    pub fn copy(&mut self, clipboard: &mut Clipboard, cursor_id: CursorId) -> bool {
        let Some(cursor) = self.cursors.get(cursor_id) else {
            return false;
        };
        if let Some(text) = cursor.selection().and_then(|s| {
            if s.end <= self.text.len() {
                Some(self.text.slice(s))
            } else {
                None
            }
        }) && clipboard.set(text.chars().collect()).is_ok()
        {
            self.undo_checkpoint();
            true
        } else {
            false
        }
    }

    pub fn cut(&mut self, clipboard: &mut Clipboard, cursor_id: CursorId) -> bool {
        if self.copy(clipboard, cursor_id) {
            self.undo_checkpoint();
            self.backspace(cursor_id, false);
            true
        } else {
            false
        }
    }

    pub fn paste(&mut self, clipboard: &mut Clipboard, cursor_id: CursorId) -> bool {
        if let Ok(s) = clipboard.get() {
            self.undo_checkpoint();
            self.enter(cursor_id, s.chars());
            true
        } else {
            false
        }
    }

    pub fn duplicate(&mut self, cursor_id: CursorId) {
        self.undo_checkpoint();

        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };

        if cursor.selection().is_some()
            && let Some(text) = cursor.selection().and_then(|s| {
                if s.end <= self.text.len() {
                    Some(self.text.slice(s))
                } else {
                    None
                }
            })
        {
            // cursor.place_at(s.end);
            self.insert_after(cursor_id, None, text.chars().collect::<Vec<_>>())
        } else {
            let coord = self.text.to_coord(cursor.pos);
            let line = self
                .text
                .line(coord[1].max(0) as usize)
                // TODO: Bad
                .map(|l| l.chars().collect::<Vec<_>>());
            if let Some(line) = line {
                let end_of_line = self.text.to_pos([0, coord[1] + 1]);
                self.insert(end_of_line, line);
            }
        }
    }

    pub fn comment(&mut self, cursor_id: CursorId) {
        self.undo_checkpoint();

        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };
        let Some(comment_syntax) = self.lang.comment_syntax.clone() else {
            return;
        };

        let lines = cursor
            .selection()
            .map(|s| self.text.to_coord(s.start)[1]..=self.text.to_coord(s.end)[1])
            .unwrap_or_else(|| {
                let coord = self.text.to_coord(cursor.pos);
                coord[1]..=coord[1]
            });
        let mut indent: Option<RopeSlice<'_>> = None;
        let mut is_comment = false;
        for line_idx in lines.clone() {
            if !self
                .text
                .start_of_line(line_idx)
                .starts_with(comment_syntax.chars())
            {
                is_comment = true;
            }
            indent = Some(match (indent, self.text.indent_of_line(line_idx)) {
                // Find common ident with existing ident (i.e: the lesser of the two)
                (Some(indent), new_indent) => new_indent.byte_slice(
                    ..indent
                        .chars()
                        .zip(new_indent.chars())
                        .take_while(|(x, y)| x == y)
                        .count(),
                ),
                (None, new_indent) => new_indent,
            });
        }
        let indent_len = indent.map_or(0, |s| s.chars().count());
        for line_idx in lines {
            let pos = self.text.to_pos([indent_len as isize, line_idx]);
            if !is_comment && self.text.slice(pos..).starts_with(comment_syntax.chars()) {
                self.remove(pos..pos + comment_syntax.len());
            } else {
                self.insert(pos, comment_syntax.chars());
            }
        }
    }

    pub fn delete_line(&mut self, cursor_id: CursorId) {
        self.undo_checkpoint();

        let Some(cursor) = self.cursors.get_mut(cursor_id) else {
            return;
        };

        if cursor.selection().is_none() {
            let pos = self.text.to_coord(cursor.pos);
            self.remove(self.text.line_range(pos[1]));
        }
    }

    pub fn line_move(&mut self, cursor_id: CursorId, dir: Dir) {
        self.undo_checkpoint();

        let Some(cursor) = self.cursors.get(cursor_id) else {
            return;
        };

        if cursor.selection().is_none() {
            let coord = self.text.to_coord(cursor.pos);
            let line_range = self.text.line_range(coord[1]);
            let line_str = self.text.slice(line_range.clone()).to_string();
            self.remove(line_range);
            let insert_pos = match dir {
                Dir::Up => self.text.to_pos([0, coord[1].saturating_sub(1)]),
                Dir::Down => self.text.to_pos([0, coord[1] + 1]),
                dir => unreachable!("{dir:?}"),
            };
            self.insert_inner(insert_pos, line_str.chars());
            let Some(cursor) = self.cursors.get_mut(cursor_id) else {
                return;
            };
            cursor.place_at(insert_pos + coord[0].max(0) as usize);
        }
    }

    pub fn start_session(&mut self) -> CursorId {
        self.cursors.insert(Cursor::default())
    }

    pub fn end_session(&mut self, cursor_id: CursorId) {
        self.cursors.remove(cursor_id);
    }

    pub fn is_same_path(&self, path: &Path) -> bool {
        self.on_disk
            .as_ref()
            .and_then(|on_disk| on_disk.path.canonicalize().ok())
            .map_or(false, |p| {
                path.canonicalize().ok().map_or(false, |path| *p == path)
            })
    }

    pub fn reload(&mut self) {
        if let Some(on_disk) = &mut self.on_disk {
            if let Ok(text) = std::fs::read_to_string(&on_disk.path) {
                self.text = Text::new(&text);
                on_disk.last_observed_modification = std::fs::metadata(&on_disk.path)
                    .and_then(|m| m.modified())
                    .unwrap_or_else(|_| SystemTime::now());
                on_disk.diverged = false;
                on_disk.unsaved = false;
                self.undo.clear();
                self.redo.clear();
                self.undo_dont_merge = false;
                self.highlights_stale = true;
                self.highlights.damage_all();
            } else {
                on_disk.diverged = true;
                on_disk.unsaved = true;
            }
        }
    }

    fn check_diverged(&mut self, needs_render: &mut bool) {
        if let Some(on_disk) = &mut self.on_disk {
            let stale = std::fs::metadata(&on_disk.path)
                .and_then(|m| m.modified())
                .map_or(true, |lm| lm > on_disk.last_observed_modification);
            if stale {
                if on_disk.unsaved {
                    if !on_disk.diverged {
                        on_disk.diverged = true;
                        *needs_render = true;
                    }
                } else {
                    self.reload();
                    *needs_render = true;
                }
            }
        }
    }

    pub fn has_changes(&mut self) -> bool {
        self.check_diverged(&mut false);
        self.on_disk
            .as_ref()
            .map(|on_disk| on_disk.diverged)
            .unwrap_or_else(|| !self.text.slice(..).is_empty())
    }

    pub fn tick(&mut self, needs_render: &mut bool) {
        self.check_diverged(needs_render);
    }

    pub fn pre_render(&mut self) {
        self.sync_highlights();
    }
}

// CLassify the character by property
fn classify(c: char) -> Option<u8> {
    match c {
        ' ' | '\t' | '\n' => None,
        c if c.is_alphanumeric() || c == '_' => Some(1),
        _ => Some(2),
    }
}

pub enum Clipboard {
    #[cfg(feature = "clipboard")]
    Global(ClipboardContext),
    Local {
        dirty: bool,
        content: String,
    },
}

impl Clipboard {
    /// Used at the end of an update to determine whether the clipboard contents need communicating to the host terminal.
    pub fn get_local_clear_dirty(&mut self) -> Option<&str> {
        if let Self::Local { dirty, content } = self
            && *dirty
        {
            *dirty = false;
            Some(content.as_str())
        } else {
            None
        }
    }

    pub fn get(&mut self) -> Result<String, ()> {
        match self {
            #[cfg(feature = "clipboard")]
            Self::Global(ctx) => ctx.get_contents().map_err(|_| ()),
            Self::Local { content, .. } => Ok(content.clone()),
        }
    }

    pub fn set_no_dirty(&mut self, text: String) -> Result<(), ()> {
        match self {
            #[cfg(feature = "clipboard")]
            Self::Global(ctx) => ctx.set_contents(text).map_err(|_| ()),
            Self::Local { .. } => Ok(*self = Self::Local {
                dirty: false,
                content: text,
            }),
        }
    }

    pub fn set(&mut self, text: String) -> Result<(), ()> {
        match self {
            #[cfg(feature = "clipboard")]
            Self::Global(ctx) => ctx.set_contents(text).map_err(|_| ()),
            Self::Local { .. } => Ok(*self = Self::Local {
                dirty: true,
                content: text,
            }),
        }
    }
}

pub struct State {
    pub buffers: DenseSlotMap<BufferId, Buffer>,
    pub terms: DenseSlotMap<TermId, Term>,
    pub tick: u64,
    pub theme: theme::Theme,
    pub clipboard: Clipboard,
    pub wakeup: Arc<tokio::sync::Notify>,
    pub needs_render: bool,
    pub bell_rung: bool,
}

impl State {
    pub fn new(_args: &Args, wakeup: Arc<tokio::sync::Notify>) -> Self {
        Self {
            buffers: DenseSlotMap::default(),
            terms: DenseSlotMap::default(),
            tick: 0,
            theme: theme::Theme::default(),
            clipboard: 'clipboard: {
                #[cfg(feature = "clipboard")]
                if let Ok(ctx) = ClipboardContext::new() {
                    break 'clipboard Clipboard::Global(ctx);
                }
                Clipboard::Local {
                    dirty: false,
                    content: String::new(),
                }
            },
            wakeup,
            needs_render: true,
            bell_rung: false,
        }
    }
}

impl State {
    pub fn get(&self, path: PathBuf) -> Result<BufferId, Error> {
        if let Some((buffer_id, _)) = self.buffers.iter().find(|(_, b)| b.is_same_path(&path)) {
            Ok(buffer_id)
        } else {
            Err(Error::NoSuchBuffer)
        }
    }

    pub fn open(&mut self, path: PathBuf) -> Result<BufferId, Error> {
        match self.get(path.clone()) {
            Ok(id) => Ok(id),
            Err(Error::NoSuchBuffer) => Ok(self.buffers.insert(Buffer::open(path)?)),
            Err(err) => Err(err),
        }
    }

    pub fn create(&mut self, path: PathBuf) -> Result<BufferId, Error> {
        match self.open(path.clone()) {
            Ok(id) => Ok(id),
            // If the file was not found, create a new file
            Err(Error::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
                let path = if path.has_root() {
                    path
                } else {
                    std::env::current_dir()?.join(path)
                };
                Ok(self.buffers.insert(Buffer::file(true, "", path)))
            }
            Err(err) => Err(err),
        }
    }

    pub fn close(&mut self, buffer: BufferId) {
        self.buffers.remove(buffer);
    }

    pub fn new_anonymous(&mut self) -> BufferId {
        self.buffers.insert(Buffer::anonymous())
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        for b in self.buffers.values_mut() {
            b.tick(&mut self.needs_render);
        }
    }

    pub fn pre_render(&mut self) {
        for b in self.buffers.values_mut() {
            b.pre_render();
        }

        for (term_id, term) in &mut self.terms {
            term.pre_render(
                &mut self.clipboard,
                &mut self.needs_render,
                &mut self.bell_rung,
            );
            if term.should_close() {
                self.close_term(term_id);
                break;
            }
        }
    }

    pub fn set_most_recent(&mut self, task: TaskId) {
        match task {
            TaskId::Buffer(buffer_id) => {
                if let Some(buffer) = self.buffers.get_mut(buffer_id) {
                    buffer.last_switch = Some(SystemTime::now());
                }
            }
            TaskId::Term(term_id) => {
                if let Some(term) = self.terms.get_mut(term_id) {
                    term.last_switch = Some(SystemTime::now());
                }
            }
        }
    }

    pub fn most_recent(&self) -> Vec<BufferId> {
        let mut most_recent = self.buffers.keys().collect::<Vec<_>>();
        most_recent.sort_by_key(|b| core::cmp::Reverse(self.buffers[*b].last_switch));
        most_recent
    }

    pub fn most_recent_tasks(&self) -> Vec<TaskId> {
        let mut most_recent = self
            .buffers
            .keys()
            .map(TaskId::Buffer)
            // Only list non-open terms, terms can only be open in one place
            .chain(
                self.terms
                    .keys()
                    .filter(|t| !self.terms[*t].is_open)
                    .map(TaskId::Term),
            )
            .collect::<Vec<_>>();
        most_recent.sort_by_key(|t| {
            core::cmp::Reverse(match t {
                TaskId::Buffer(b) => self.buffers[*b].last_switch,
                TaskId::Term(t) => self.terms[*t].last_switch,
            })
        });
        most_recent
    }

    pub fn create_term(&mut self, term: Term) -> TermWindow {
        let term_id = self.terms.insert(term);
        self.switch_term(term_id)
    }

    pub fn close_term(&mut self, term: TermId) {
        if let Some(term) = self.terms.remove(term) {
            term.close(self);
        }
    }

    pub fn close_term_window(&mut self, term_id: TermId) {
        if let Some(term) = self.terms.get_mut(term_id) {
            if term.is_open {
                term.is_open = false;
            } else {
                self.close_term(term_id);
            }
        }
    }

    pub fn switch_term(&mut self, term: TermId) -> TermWindow {
        assert!(!self.terms[term].is_open);
        self.terms[term].is_open = false;
        TermWindow::new(term)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum TaskId {
    Buffer(BufferId),
    Term(TermId),
}
