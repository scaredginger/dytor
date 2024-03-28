use std::collections::VecDeque;

use super::{ReadErr, ReadResult, Rx, Tx, WriteErr, WriteResult};

#[derive(Default)]
pub struct Unbounded<T> {
    queue: VecDeque<T>,
}

impl<T> Tx<T> for &mut Unbounded<T> {
    fn send(&mut self, value: T) -> WriteResult<T> {
        self.queue.push_back(value);
        Ok(())
    }
}

impl<T> Rx<T> for &mut Unbounded<T> {
    fn recv(&mut self) -> ReadResult<T> {
        self.queue.pop_front().ok_or(ReadErr::Empty)
    }
}

pub struct Bounded<T> {
    queue: VecDeque<T>,
    capacity: usize,
}

impl<T> Bounded<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
}

impl<T> Tx<T> for &mut Bounded<T> {
    fn send(&mut self, value: T) -> WriteResult<T> {
        if self.queue.len() >= self.capacity {
            return Err(WriteErr::Full(value));
        }
        self.queue.push_back(value);
        Ok(())
    }
}

impl<T> Rx<T> for &mut Bounded<T> {
    fn recv(&mut self) -> ReadResult<T> {
        self.queue.pop_front().ok_or(ReadErr::Empty)
    }
}
