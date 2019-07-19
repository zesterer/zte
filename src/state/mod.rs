use std::{
    path::PathBuf,
    ops::Deref,
    io,
};
use crate::{
    buffer::shared::SharedBufferError,
    SharedBufferRef,
    Buffer,
    BufferMut,
    Event,
};

#[derive(Debug)]
pub enum StateError {
    Shared(SharedBufferError),
}

impl From<SharedBufferError> for StateError {
    fn from(err: SharedBufferError) -> Self {
        StateError::Shared(err)
    }
}

pub struct State {
    recent: Vec<SharedBufferRef>,
}

impl State {
    pub fn from_paths(paths: impl Iterator<Item=PathBuf>) -> (Self, Vec<StateError>) {
        let mut errors = Vec::new();

        let this = Self {
            recent: paths
                .filter_map(|path| match SharedBufferRef::open(path) {
                    Ok(buf) => Some(buf),
                    Err(err) => {
                        errors.push(err.into());
                        None
                    },
                })
                .collect(),
        };

        (this, errors)
    }

    pub fn recent(&self) -> &[SharedBufferRef] {
        &self.recent
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            recent: Vec::new(),
        }
    }
}
