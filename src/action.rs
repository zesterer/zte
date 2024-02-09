use crate::{
    state::State,
    terminal::TerminalEvent,
};
use crossterm::event::{KeyEvent, KeyCode, KeyEventKind, KeyModifiers};

#[derive(Clone, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug)]
pub enum Action {
    Char(char), // Insert a character
    Backspace, // Backspace a character
    Move(Dir), // Move the cursor
    Cancel, // Cancels the current context
    Go, // Search, accept, or select the current option
    Quit, // Quit the application
    OpenPrompt, // Open the command prompt
    Show(String), // Display some arbitrary text to the user
}

pub enum Event {
    // The incoming event is an action generated by some other internal component.
    Action(Action),
    // The incoming event is a raw user input.
    Raw(RawEvent),
}

impl Event {
    pub fn from_raw(e: TerminalEvent) -> Self {
        Self::Raw(RawEvent(e))
    }
    
    /// Turn the event into an action (if possible).
    ///
    /// The translation function allows elements to translate raw events into their own context-specific actions.
    pub fn to_action(&self, translate: impl FnOnce(&RawEvent) -> Option<Action>) -> Option<Action> {
        match self {
            Self::Action(a) => Some(a.clone()),
            Self::Raw(te) => translate(te),
        }
    }
}

pub struct RawEvent(TerminalEvent);

impl RawEvent {
    pub fn to_char(&self) -> Option<char> {
        match &self.0 {
            TerminalEvent::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => match code {
                KeyCode::Char(c) => Some(*c),
                KeyCode::Backspace => Some('\x08'),
                KeyCode::Enter => Some('\n'),
                _ => None,
            },
            _ => None,
        }
    }
    
    pub fn to_move(&self) -> Option<Dir> {
        match &self.0 {
            TerminalEvent::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => match code {
                KeyCode::Left => Some(Dir::Left),
                KeyCode::Right => Some(Dir::Right),
                KeyCode::Up => Some(Dir::Up),
                KeyCode::Down => Some(Dir::Down),
                _ => None,
            },
            _ => None,
        }
    }
    
    pub fn is_go(&self) -> bool {
        matches!(&self.0, TerminalEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }))
    }
    
    pub fn is_prompt(&self) -> bool {
        matches!(&self.0, TerminalEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::ALT,
            kind: KeyEventKind::Press,
            ..
        }))
    }
    
    pub fn is_cancel(&self) -> bool {
        matches!(&self.0, TerminalEvent::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }))
    }
}