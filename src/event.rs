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
    Escape,
    Quit,
}
