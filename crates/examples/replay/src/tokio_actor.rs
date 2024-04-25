use std::future::Future;
use std::pin::Pin;
use std::thread;

use common::anyhow;
use common::rian::{register_actor, Actor, InitArgs, UniquelyNamed};
use tokio::sync::mpsc;
use tokio::task::LocalSet;

#[derive(UniquelyNamed)]
pub struct TokioSingleThread {
    thread: Option<thread::JoinHandle<()>>,
    task_tx: mpsc::UnboundedSender<LazyDynFut>,
}

register_actor!(TokioSingleThread);

impl Actor for TokioSingleThread {
    type Config = ();

    fn init(_args: InitArgs<Self>, _cfg: ()) -> anyhow::Result<Self> {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        let thread = thread::spawn(move || run_async_event_loop(task_rx));
        Ok(Self {
            thread: Some(thread),
            task_tx,
        })
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
        let thread = self.thread.take().unwrap();
        thread.join().unwrap();
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
        while let Some(f) = task_rx.recv().await {
            tokio::task::spawn_local(f());
        }
    });
    rt.block_on(local);
}
