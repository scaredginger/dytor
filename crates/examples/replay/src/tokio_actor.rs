use std::future::Future;
use std::pin::Pin;
use std::thread;
use tokio::signal::unix::{signal, SignalKind};

use common::anyhow;
use common::rian::{register_actor, Actor, InitArgs, UniquelyNamed};
use tokio::select;
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

    fn init(args: InitArgs<Self>, _cfg: ()) -> anyhow::Result<Self> {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        let acc = args.accessor();
        let thread = thread::spawn(move || {
            run_async_event_loop(task_rx);
            drop(acc);
        });
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
        self.thread.take().unwrap().join().unwrap();
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
