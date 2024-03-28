pub mod local;

pub trait Rx<T: 'static + Send> {
    fn recv(&mut self) -> ReadResult<T>;
}

pub trait Tx<T: 'static + Send> {
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
