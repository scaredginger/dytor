pub mod local;
mod std_mpsc;

pub trait Queue<T: Send> {
    type Rx: Rx<T>;
    type Tx: Tx<T>;

    fn channel() -> (Self::Tx, Self::Rx);
}

pub trait Rx<T: Send>: 'static + Send {
    fn recv(&mut self) -> ReadResult<T>;
}

pub trait Tx<T: Send>: 'static + Send {
    fn send(&self, value: T) -> WriteResult<T>;
}

#[derive(Debug)]
pub enum ReadErr {
    Empty,
    Finished,
}

#[derive(Debug)]
pub enum WriteErr<T> {
    Finished(T),
    Full(T),
}

pub type ReadResult<T> = Result<T, ReadErr>;
pub type WriteResult<T> = Result<(), WriteErr<T>>;
