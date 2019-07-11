use std::{
    thread,
    sync::mpsc::{channel, Receiver},
};
use crossterm::{
    TerminalInput,
    InputEvent,
    KeyEvent,
};
use crate::Event;

pub fn begin_reading(input: TerminalInput) -> Receiver<Event> {
    let (tx, rx) = channel();

    thread::spawn(move || for event in input.read_sync() {
        let event = match event {
            InputEvent::Keyboard(KeyEvent::Ctrl('q')) => Event::Quit,
            _ => continue,
        };
        tx.send(event).unwrap();
    });

    rx
}
