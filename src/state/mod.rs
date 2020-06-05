use std::{
    path::PathBuf,
    ops::Deref,
    io,
    collections::HashMap,
};
use crate::{
    buffer::shared::{
        SharedBuffer,
        SharedBufferError,
        BufferGuard,
    },
    BufferId,
    BufferHandle,
    Cursor,
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

#[derive(Default)]
pub struct State {
    buffers: HashMap<BufferId, SharedBuffer>,
    id_counter: usize,
    recent: Vec<BufferHandle>,
}

impl State {
    pub fn from_paths(paths: impl Iterator<Item=PathBuf>) -> (Self, Vec<StateError>) {
        let mut errors = Vec::new();

        let mut this = Self::default();

        for path in paths {
            match SharedBuffer::open(path) {
                Ok(buf) => { this.insert_buffer(buf); },
                Err(err) => errors.push(err.into()),
            }
        }

        (this, errors)
    }

    pub fn new_handle(&mut self, buffer_id: BufferId) -> Option<BufferHandle> {
        let cursor_id = self.buffers
            .get_mut(&buffer_id)?
            .insert_cursor(Cursor::default());
        Some(BufferHandle {
            buffer_id,
            cursor_id,
        })
    }

    pub fn clone_handle(&mut self, handle: BufferHandle) -> Option<BufferHandle> {
        let buf = self
            .get_buffer(handle)?
            .buffer;

        let cursor = buf
            .cursor(handle.cursor_id)
            .clone();

        let cursor_id = buf.insert_cursor(cursor);

        Some(BufferHandle {
            cursor_id,
            ..handle
        })
    }

    pub fn insert_buffer(&mut self, mut buf: SharedBuffer) -> BufferId {
        self.id_counter += 1;
        let id = BufferId(self.id_counter);
        self.buffers.insert(id, buf);
        let handle = self.new_handle(id).unwrap();
        self.recent.push(handle);
        id
    }

    pub fn new_empty_buffer(&mut self) -> BufferId {
        self.insert_buffer(SharedBuffer::default())
    }

    pub fn get_buffer(&mut self, handle: BufferHandle) -> Option<BufferGuard> {
        Some(BufferGuard {
            buffer: self.buffers.get_mut(&handle.buffer_id)?,
            cursor_id: handle.cursor_id,
        })
    }

    pub fn get_shared_buffer(&mut self, id: BufferId) -> Option<&mut SharedBuffer> {
        self.buffers.get_mut(&id)
    }

    pub fn buffers(&self) -> impl Iterator<Item=BufferId> + ExactSizeIterator<Item=BufferId> + '_ {
        self.buffers.keys().copied()
    }

    pub fn recent_buffers(&self) -> impl Iterator<Item=BufferHandle> + ExactSizeIterator<Item=BufferHandle> + '_ {
        self.recent.iter().rev().copied()
    }

    pub fn set_recent_buffer(&mut self, handle: BufferHandle) {
        self.recent.retain(|h| h.buffer_id != handle.buffer_id);
        self.recent.push(handle);
    }
}
