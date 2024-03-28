use std::{collections::VecDeque, num::NonZeroUsize};

use super::{ReadErr, ReadResult, WriteErr, WriteResult};

pub struct LocalQueue<T> {
    queue: VecDeque<T>,
    capacity: Option<NonZeroUsize>,
}

impl<T: 'static> LocalQueue<T> {
    fn send(&mut self, value: T) -> WriteResult<T> {
        if self.capacity.is_some_and(|c| c.get() >= self.queue.len()) {
            return Err(WriteErr::Full(value));
        }
        self.queue.push_back(value);
        Ok(())
    }

    fn recv(&mut self) -> ReadResult<T> {
        self.queue.pop_front().ok_or(ReadErr::Empty)
    }

    pub fn bounded(capacity: NonZeroUsize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity.get()),
            capacity: Some(capacity),
        }
    }

    pub fn unbounded() -> Self {
        Self {
            queue: VecDeque::new(),
            capacity: None,
        }
    }
}
