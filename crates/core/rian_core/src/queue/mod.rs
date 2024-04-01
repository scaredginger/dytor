pub mod local;
mod std_mpsc;

pub trait Queue<T: Send> {
    type Rx: Rx<T>;
    type Tx: Tx<T>;

    fn channel() -> (Self::Tx, Self::Rx);
}

pub trait Rx<T: Send>: Send {
    fn recv(&mut self) -> ReadResult<T>;
}

pub trait Tx<T: Send>: Send {
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
