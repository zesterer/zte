use std::{
    thread,
    sync::mpsc::{channel, Receiver},
};
use crossterm::{
    TerminalInput,
    InputEvent,
    KeyEvent,
};
use crate::{
    Event,
    Dir,
};

pub fn begin_reading(input: TerminalInput) -> Receiver<Event> {
    let (tx, rx) = channel();

    thread::spawn(move || for event in input.read_sync() {
        let event = match event {
            InputEvent::Keyboard(KeyEvent::Ctrl('q')) => Event::Quit,
            InputEvent::Keyboard(KeyEvent::Char(c)) => Event::Insert(c),
            InputEvent::Keyboard(KeyEvent::Left) => Event::CursorMove(Dir::Left),
            InputEvent::Keyboard(KeyEvent::Right) => Event::CursorMove(Dir::Right),
            InputEvent::Keyboard(KeyEvent::Up) => Event::CursorMove(Dir::Up),
            InputEvent::Keyboard(KeyEvent::Down) => Event::CursorMove(Dir::Down),
            InputEvent::Keyboard(KeyEvent::Backspace) => Event::Backspace,
            _ => continue,
        };
        tx.send(event).unwrap();
    });

    rx
}
