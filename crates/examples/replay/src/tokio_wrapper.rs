use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;
use tokio::signal::unix::{signal, SignalKind};

use common::dytor::register_resource;
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::LocalSet;

#[derive(Clone)]
pub struct TokioSingleThread {
    task_tx: Arc<mpsc::UnboundedSender<LazyDynFut>>,
}

register_resource!(|| {
    let (task_tx, task_rx) = mpsc::unbounded_channel();
    thread::spawn(move || {
        run_async_event_loop(task_rx);
    });
    TokioSingleThread {
        task_tx: Arc::new(task_tx),
    }
});

impl TokioSingleThread {
    pub fn spawn_with<Fut: Future<Output = ()> + 'static>(
        &self,
        f: impl FnOnce() -> Fut + Send + 'static,
    ) {
        self.spawn_with_boxed(Box::new(move || Box::pin(f())));
    }

    pub fn spawn_with_boxed(&self, f: LazyDynFut) {
        self.task_tx.send(f).unwrap();
    }
}

pub type LazyDynFut = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>> + Send + 'static>;

fn run_async_event_loop(mut task_rx: mpsc::UnboundedReceiver<LazyDynFut>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("TokioRuntimeWorker")
        .build()
        .unwrap();

    let local = LocalSet::new();
    local.spawn_local(async move {
        let mut signal = signal(SignalKind::interrupt()).unwrap();
        loop {
            select! {
                biased;

                f = task_rx.recv() => {
                    match f {
                        Some(f) => tokio::task::spawn_local(f()),
                        None => break,
                    };
                }
                _ = signal.recv() => {
                    break;
                }
            }
        }
    });
    rt.block_on(local);
}
