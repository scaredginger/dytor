use std::future::Future;
use std::pin::Pin;
use std::thread;

use common::anyhow;
use common::rian::{register_actor, Actor, InitArgs, UniquelyNamed};
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use tokio_util::sync::CancellationToken;

#[derive(UniquelyNamed)]
pub struct TokioSingleThread {
    thread: Option<thread::JoinHandle<()>>,
    task_tx: mpsc::UnboundedSender<LazyDynFut>,
    token: CancellationToken,
}

register_actor!(TokioSingleThread);

impl Actor for TokioSingleThread {
    type Config = ();

    fn init(_args: InitArgs<Self>, _cfg: ()) -> anyhow::Result<Self> {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        let token = CancellationToken::new();
        let token2 = token.clone();
        let thread = thread::spawn(move || run_async_event_loop(task_rx, token2));
        Ok(Self {
            thread: Some(thread),
            task_tx,
            token,
        })
    }

    fn is_finished(&self) -> bool {
        true
    }

    fn stop(&mut self) {
        self.token.cancel();
        let thread = self.thread.take().unwrap();
        thread.join().unwrap();
    }
}

impl TokioSingleThread {
    pub fn spawn_with<Fut: Future<Output = ()> + 'static>(
        &mut self,
        f: impl FnOnce() -> Fut + Send + 'static,
    ) {
        self.spawn_with_boxed(Box::new(move || Box::pin(f())));
    }

    pub fn spawn_with_boxed(&mut self, f: LazyDynFut) {
        self.task_tx.send(f).unwrap();
    }
}

impl Drop for TokioSingleThread {
    fn drop(&mut self) {
        assert!(self.thread.is_none());
    }
}

pub type LazyDynFut = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>> + Send + 'static>;

fn run_async_event_loop(
    mut task_rx: mpsc::UnboundedReceiver<LazyDynFut>,
    token: CancellationToken,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("TokioRuntimeWorker")
        .build()
        .unwrap();

    let local = LocalSet::new();
    local.spawn_local(async move {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                f = task_rx.recv() => {
                    match f {
                        Some(f) => tokio::task::spawn_local(f()),
                        None => break,
                    };
                }
            }
        }
    });
    rt.block_on(local);
}
