use std::sync::mpsc::{channel, Receiver, RecvError, SendError, Sender, TryRecvError};

use super::{Queue, ReadErr, ReadResult, Rx, Tx, WriteErr, WriteResult};

struct StdMpsc;

impl<T: 'static + Send> Queue<T> for StdMpsc {
    type Rx = Receiver<T>;

    type Tx = Sender<T>;

    fn channel() -> (Self::Tx, Self::Rx) {
        channel()
    }
}

impl<T: 'static + Send> Rx<T> for Receiver<T> {
    fn recv(&mut self) -> ReadResult<T> {
        Receiver::recv(self).map_err(|_| ReadErr::Finished)
    }
}

impl<T: 'static + Send> Tx<T> for Sender<T> {
    fn send(&mut self, value: T) -> WriteResult<T> {
        Sender::send(self, value).map_err(|SendError(v)| WriteErr::Finished(v))
    }
}
