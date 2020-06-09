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
        let events = match event.unwrap() {
            // Buffer movement

            InputEvent::Key(KeyEvent::Left) => vec![Event::CursorMove(Dir::Left)],
            InputEvent::Key(KeyEvent::Right) => vec![Event::CursorMove(Dir::Right)],
            InputEvent::Key(KeyEvent::Up) => vec![Event::CursorMove(Dir::Up)],
            InputEvent::Key(KeyEvent::Down) => vec![Event::CursorMove(Dir::Down)],

            // Buffer editing

            InputEvent::Key(KeyEvent::Char(c)) => vec![Event::Insert(c)],
            InputEvent::Key(KeyEvent::Backspace) => vec![Event::Backspace],
            InputEvent::Key(KeyEvent::Delete) => vec![Event::Delete],
            InputEvent::Key(KeyEvent::Ctrl('z')) => vec![Event::Undo],
            InputEvent::Key(KeyEvent::Ctrl('y')) => vec![Event::Redo],
            InputEvent::Key(KeyEvent::Esc) => vec![Event::Escape],
            InputEvent::Key(KeyEvent::PageUp) => vec![Event::PageMove(Dir::Up)],
            InputEvent::Key(KeyEvent::PageDown) => vec![Event::PageMove(Dir::Down)],

            // Buffer manipulation

            InputEvent::Key(KeyEvent::Ctrl('b')) => vec![Event::OpenSwitcher],
            InputEvent::Key(KeyEvent::Ctrl('o')) => vec![Event::OpenOpener],
            InputEvent::Key(KeyEvent::Ctrl('q')) => vec![Event::CloseBuffer],

            // Buffer actions

            InputEvent::Key(KeyEvent::Ctrl('s')) => vec![Event::SaveBuffer],
            InputEvent::Key(KeyEvent::Ctrl('x')) => vec![Event::Cut],
            InputEvent::Key(KeyEvent::Ctrl('c')) => vec![Event::Copy],
            InputEvent::Key(KeyEvent::Ctrl('v')) => vec![Event::Paste],
            InputEvent::Key(KeyEvent::Ctrl('d')) => vec![Event::DuplicateLine],

            // Tile movement

            InputEvent::Key(KeyEvent::Alt('a')) => vec![Event::SwitchEditor(Dir::Left)],
            InputEvent::Key(KeyEvent::Alt('d')) => vec![Event::SwitchEditor(Dir::Right)],
            InputEvent::Key(KeyEvent::Alt('w')) => vec![Event::SwitchEditor(Dir::Up)],
            InputEvent::Key(KeyEvent::Alt('s')) => vec![Event::SwitchEditor(Dir::Down)],

            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 68] => vec![Event::SwitchEditor(Dir::Left)],
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 67] => vec![Event::SwitchEditor(Dir::Right)],
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 65] => vec![Event::SwitchEditor(Dir::Up)],
            InputEvent::Unsupported(event) if event == &[27, 91, 49, 59, 51, 66] => vec![Event::SwitchEditor(Dir::Down)],

            // Tile manipulation

            InputEvent::Key(KeyEvent::Alt('q')) => vec![Event::CloseEditor],
            InputEvent::Key(KeyEvent::Alt('A')) => vec![Event::NewEditor(Dir::Left)],
            InputEvent::Key(KeyEvent::Alt('D')) => vec![Event::NewEditor(Dir::Right)],
            InputEvent::Key(KeyEvent::Alt('W')) => vec![Event::NewEditor(Dir::Up)],
            InputEvent::Key(KeyEvent::Alt('S')) => vec![Event::NewEditor(Dir::Down)],

            // Misc
            InputEvent::Key(KeyEvent::Ctrl('p')) => vec![Event::OpenPrompt],
            InputEvent::Key(KeyEvent::Ctrl('p')) => vec![Event::NewTerminal(Dir::Right)],

            InputEvent::Unsupported(event) => {
                log::info!("Unsupported event: {:?}", event);
                continue;
            },
            _ => continue,
        };

        for event in events {
            tx.send(event).unwrap();
        }
    });

    rx
}
