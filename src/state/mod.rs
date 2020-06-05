use std::{
    path::PathBuf,
    ops::Deref,
    io,
    collections::HashMap,
    sync::Arc,
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
    buffers: HashMap<BufferId, (SharedBuffer, Arc<()>)>,
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

    pub fn open_file(&mut self, path: PathBuf) -> Result<BufferHandle, SharedBufferError> {
        let full_path = path.canonicalize().unwrap();
        self
            .buffers
            .iter()
            .find(|(_, (buf, _))| buf.path.as_ref().map(|p| p == &full_path).unwrap_or(false))
            .map(|(id, _)| Ok(*id))
            .unwrap_or_else(|| Ok(self.insert_buffer(SharedBuffer::open(path)?)))
            .map(|buf| self.new_handle(buf).unwrap())
    }

    pub fn new_handle(&mut self, buffer_id: BufferId) -> Option<BufferHandle> {
        let buf = self.buffers.get_mut(&buffer_id)?;
        let cursor_id = buf.0.insert_cursor(Cursor::default());
        Some(BufferHandle {
            buffer_id,
            cursor_id,
            rc: buf.1.clone(),
        })
    }

    pub fn duplicate_handle(&mut self, handle: &BufferHandle) -> Option<BufferHandle> {
        let buf = self.get_buffer(handle)?;

        let cursor = buf
            .buffer
            .cursor(handle.cursor_id)
            .clone();

        let cursor_id = buf.buffer.insert_cursor(cursor);

        Some(BufferHandle {
            cursor_id,
            buffer_id: handle.buffer_id,
            rc: handle.rc.clone(),
        })
    }

    pub fn insert_buffer(&mut self, buf: SharedBuffer) -> BufferId {
        self.id_counter += 1;
        let id = BufferId(self.id_counter);
        self.buffers.insert(id, (buf, Arc::new(())));
        let handle = self.new_handle(id).unwrap();
        self.recent.push(handle);
        id
    }

    pub fn new_empty_buffer(&mut self) -> BufferId {
        self.insert_buffer(SharedBuffer::default())
    }

    pub fn get_buffer(&mut self, handle: &BufferHandle) -> Option<BufferGuard> {
        Some(BufferGuard {
            buffer: &mut self.buffers.get_mut(&handle.buffer_id)?.0,
            cursor_id: handle.cursor_id,
        })
    }

    pub fn get_shared_buffer(&mut self, id: BufferId) -> Option<&mut SharedBuffer> {
        self.buffers.get_mut(&id).map(|(buf, _)| buf)
    }

    pub fn buffers(&self) -> impl Iterator<Item=BufferId> + ExactSizeIterator<Item=BufferId> + '_ {
        self.buffers.keys().copied()
    }

    pub fn recent_buffers(&self) -> impl Iterator<Item=&BufferHandle> + ExactSizeIterator<Item=&BufferHandle> + '_ {
        self.recent.iter().rev()
    }

    pub fn set_recent_buffer(&mut self, handle: BufferHandle) {
        self.recent.retain(|h| h.buffer_id != handle.buffer_id);
        self.recent.push(handle);
    }
}
