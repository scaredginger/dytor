pub mod local;

pub trait Rx<T> {
    fn recv(&mut self) -> ReadResult<T>;
}

pub trait Tx<T> {
    fn send(&mut self, value: T) -> WriteResult<T>;
}

pub enum ReadErr {
    Empty,
    Finished,
}

pub enum WriteErr<T> {
    Finished(T),
    Full(T),
}

pub type ReadResult<T> = Result<T, ReadErr>;
pub type WriteResult<T> = Result<(), WriteErr<T>>;
