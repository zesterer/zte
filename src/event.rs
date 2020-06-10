use std::path::PathBuf;
use vek::*;
use crate::BufferHandle;

#[derive(Copy, Clone, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl From<Dir> for Vec2<isize> {
    fn from(dir: Dir) -> Vec2<isize> {
        match dir {
            Dir::Left => Vec2::new(-1, 0),
            Dir::Right => Vec2::new(1, 0),
            Dir::Up => Vec2::new(0, -1),
            Dir::Down => Vec2::new(0, 1),
        }
    }
}

impl Dir {
    pub fn is_forward(&self) -> bool {
        matches!(self, Dir::Right | Dir::Down)
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Insert(char),
    Backspace,
    BackspaceWord,
    Delete,
    Undo,
    Redo,
    CursorMove(Dir, bool),
    CursorJump(Dir, bool),
    PageMove(Dir),
    SwitchEditor(Dir),
    NewEditor(Dir),
    NewTerminal(Dir),
    CloseEditor,
    OpenPrompt,
    OpenSwitcher,
    OpenOpener,
    OpenFile(PathBuf),
    CloseMenu,
    SwitchBuffer(BufferHandle),
    CloseBuffer,
    SaveBuffer,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Escape,
    Quit,
}
