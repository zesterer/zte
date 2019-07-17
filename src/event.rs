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
    SwitchEditor(Dir),
    NewEditor(Dir),
    CloseEditor,
    Quit,
}
