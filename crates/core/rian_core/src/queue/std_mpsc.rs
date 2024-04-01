use std::sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError};

use super::{Queue, ReadErr, ReadResult, Rx, Tx, WriteErr, WriteResult};

struct StdMpsc;

impl<T: Send> Queue<T> for StdMpsc {
    type Rx = Receiver<T>;

    type Tx = Sender<T>;

    fn channel() -> (Self::Tx, Self::Rx) {
        channel()
    }
}

impl<T: Send> Rx<T> for Receiver<T> {
    fn recv(&mut self) -> ReadResult<T> {
        self.try_recv().map_err(|e| match e {
            TryRecvError::Empty => ReadErr::Empty,
            TryRecvError::Disconnected => ReadErr::Finished,
        })
    }
}

impl<T: Send> Tx<T> for Sender<T> {
    fn send(&mut self, value: T) -> WriteResult<T> {
        Sender::send(self, value).map_err(|SendError(v)| WriteErr::Finished(v))
    }
}
