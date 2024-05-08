use std::cmp::Ordering;
use std::collections::BinaryHeap;

use common::anyhow::Result;
use common::rian::{register_actor, Accessor, Actor, InitArgs, UniquelyNamed};
use tokio::sync::oneshot;
pub use tokio_stream::StreamExt;

use crate::{DynStream, Event, Producer, TokioSingleThread, UntypedBox};

#[derive(UniquelyNamed)]
pub struct Synchronizer {}

register_actor!(Synchronizer);

impl Actor for Synchronizer {
    type Config = ();

    fn init(mut args: InitArgs<Self>, _config: Self::Config) -> Result<Self> {
        let sources = args.query().all_accessors().collect();
        let runtime = args.get_resource::<TokioSingleThread>();
        let runtime2 = runtime.clone();
        runtime.spawn_with(move || background_task(sources, runtime2));
        Ok(Self {})
    }
}
struct HeapEntry {
    next_event: Event<UntypedBox>,
    acc: Accessor<dyn Producer>,
    stream: DynStream,
}

impl Eq for HeapEntry {}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.next_event.timestamp == other.next_event.timestamp
            && self.next_event.tie_break == other.next_event.tie_break
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.next_event.timestamp.cmp(&other.next_event.timestamp) {
            Ordering::Equal => (),
            x => return x,
        };

        self.next_event.tie_break.cmp(&other.next_event.tie_break)
    }
}

async fn background_task(producers: Vec<Accessor<dyn Producer>>, runtime: TokioSingleThread) {
    let mut heap = BinaryHeap::with_capacity(producers.len());

    let futures: Vec<_> = producers
        .into_iter()
        .map(|acc| {
            let (tx, rx) = oneshot::channel();
            let runtime = runtime.clone();
            acc.send(|args, p| {
                tx.send(p.create_stream(args, runtime))
                    .unwrap_or_else(|_| panic!("Could not send"))
            });
            (acc, rx)
        })
        .collect();

    let heap_ref = &mut heap;
    let mut stream_stream = tokio_stream::iter(futures).then(move |(acc, fut)| {
        Box::pin(async move {
            let mut stream = fut.await.unwrap_or_else(|_| panic!("No stream returned"));
            let next_event = stream.next().await?;
            Some(HeapEntry {
                acc,
                next_event,
                stream,
            })
        })
    });

    while let Some(entry) = stream_stream.next().await {
        if let Some(entry) = entry {
            heap_ref.push(entry);
        }
    }

    while let Some(HeapEntry {
        next_event,
        acc,
        mut stream,
    }) = heap.pop()
    {
        let Event {
            timestamp,
            tie_break,
            obj,
        } = next_event;
        acc.send(move |args, p| {
            p.process_event(
                args,
                Event {
                    timestamp,
                    tie_break,
                    obj,
                },
            )
        });

        let ev = stream.next().await;
        if let Some(ev) = ev {
            heap.push(HeapEntry {
                next_event: ev,
                acc,
                stream,
            });
        }
    }
}
