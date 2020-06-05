use std::path::PathBuf;
use crate::BufferHandle;

#[derive(Copy, Clone, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug)]
pub enum Event {
    Insert(char),
    Backspace,
    Delete,
    CursorMove(Dir),
    PageMove(Dir),
    SwitchEditor(Dir),
    NewEditor(Dir),
    CloseEditor,
    OpenPrompt,
    OpenSwitcher,
    OpenOpener,
    OpenFile(PathBuf),
    CloseMenu,
    SwitchBuffer(BufferHandle),
    SaveBuffer,
    Cut,
    Copy,
    Paste,
    DuplicateLine,
    Escape,
    Quit,
}
