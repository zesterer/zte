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
    CursorMove(Dir),
    SwitchEditor(Dir),
    Quit,
}
