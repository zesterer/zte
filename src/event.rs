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

#[derive(Clone, Debug)]
pub enum Event {
    Insert(char),
    Backspace,
    BackspaceWord,
    Delete,
    Undo,
    Redo,
    CursorMove(Dir),
    CursorJump(Dir),
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
    DuplicateLine,
    Escape,
    Quit,
}
