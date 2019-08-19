use std::{
    thread,
    sync::mpsc::{channel, Receiver},
    io::stdin,
};
/*
use crossterm::{
    TerminalInput,
    InputEvent,
    KeyEvent,
};
*/
use termion::{
    input::{MouseTerminal, TermRead},
    event::{Event as InputEvent, Key as KeyEvent, MouseEvent},
};
use crate::{
    Event,
    Dir,
};

pub fn begin_reading() -> Receiver<Event> {
    let (tx, rx) = channel();

    thread::spawn(move || for event in stdin().events() {
        let event = match event.unwrap() {
            InputEvent::Key(KeyEvent::Ctrl('q')) => Event::Quit,
            InputEvent::Key(KeyEvent::Char(c)) => Event::Insert(c),

            InputEvent::Key(KeyEvent::Left) => Event::CursorMove(Dir::Left),
            InputEvent::Key(KeyEvent::Right) => Event::CursorMove(Dir::Right),
            InputEvent::Key(KeyEvent::Up) => Event::CursorMove(Dir::Up),
            InputEvent::Key(KeyEvent::Down) => Event::CursorMove(Dir::Down),

            InputEvent::Key(KeyEvent::Backspace) => Event::Backspace,
            InputEvent::Key(KeyEvent::Delete) => Event::Delete,
            InputEvent::Key(KeyEvent::Esc) => Event::Escape,
            InputEvent::Key(KeyEvent::PageUp) => Event::PageMove(Dir::Up),
            InputEvent::Key(KeyEvent::PageDown) => Event::PageMove(Dir::Down),

            InputEvent::Key(KeyEvent::Ctrl('w')) => Event::CloseEditor,
            InputEvent::Key(KeyEvent::Ctrl('t')) => Event::NewEditor(Dir::Right),
            InputEvent::Key(KeyEvent::Ctrl('n')) => Event::NewEditor(Dir::Down),
            InputEvent::Key(KeyEvent::Ctrl('p')) => Event::OpenPrompt,
            InputEvent::Key(KeyEvent::Ctrl('o')) => Event::OpenSwitcher,
            InputEvent::Key(KeyEvent::Ctrl('s')) => Event::SaveBuffer,
            InputEvent::Key(KeyEvent::Ctrl('x')) => Event::Cut,
            InputEvent::Key(KeyEvent::Ctrl('c')) => Event::Copy,
            InputEvent::Key(KeyEvent::Ctrl('v')) => Event::Paste,

            InputEvent::Key(KeyEvent::Alt('h')) => Event::SwitchEditor(Dir::Left),
            InputEvent::Key(KeyEvent::Alt('k')) => Event::SwitchEditor(Dir::Right),
            InputEvent::Key(KeyEvent::Alt('u')) => Event::SwitchEditor(Dir::Up),
            InputEvent::Key(KeyEvent::Alt('j')) => Event::SwitchEditor(Dir::Down),
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 68] => Event::SwitchEditor(Dir::Left),
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 67] => Event::SwitchEditor(Dir::Right),
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 65] => Event::SwitchEditor(Dir::Up),
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 66] => Event::SwitchEditor(Dir::Down),

            InputEvent::Unsupported(event) => {
                log::info!("Unsupported event: {:?}", event);
                continue;
            },
            _ => continue,
        };
        tx.send(event).unwrap();
    });

    rx
}
