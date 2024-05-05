use std::sync::mpsc;

pub fn channel<T>() -> (Tx<T>, Rx<T>) {
    let (tx, rx) = mpsc::channel();
    (Tx(tx), Rx(rx))
}

pub struct Tx<T>(mpsc::Sender<T>);
pub struct Rx<T>(mpsc::Receiver<T>);

#[derive(Debug)]
pub struct SendError;

impl<T: 'static + Send> Tx<T> {
    pub fn send(&self, value: T) -> Result<(), SendError> {
        self.0.send(value).map_err(|_| SendError)
    }
}

impl<T: 'static + Send> Rx<T> {
    pub fn recv(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}

impl<T> Clone for Tx<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
