use std::any::Any;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::pin::Pin;

use common::anyhow::{anyhow, Result};
use common::chrono::{DateTime, Utc};
use common::rian::lookup::Key;
use common::rian::{register_actor, Accessor, Actor, InitArgs, UniquelyNamed};
use itertools::Itertools as _;
use tokio::sync::oneshot;
use tokio_stream::{Stream, StreamExt};

use crate::TokioSingleThread;

pub struct Event<T> {
    timestamp: DateTime<Utc>,
    tie_break: u64,
    obj: T,
}

#[derive(UniquelyNamed)]
pub struct Synchronizer {}

register_actor!(Synchronizer);

impl Actor for Synchronizer {
    type Config = ();

    fn init(mut args: InitArgs<Self>, _config: Self::Config) -> Result<Self> {
        let key: Key<TokioSingleThread> = args
            .query()
            .all_keys()
            .exactly_one()
            .map_err(|_| anyhow!("Multiple tokio runtimes"))?;
        let sources = args.query().all_accessors().collect();
        args.send_msg(key, move |_, obj| {
            obj.spawn_with(move || background_task(sources))
        });
        Ok(Self {})
    }
}

mod private {
    use super::*;
    pub trait Sealed {}
    impl<T: TypedProducer> Sealed for T {}
}

pub trait TypedProducer {
    type Item: Send + 'static;

    fn event_stream(&self) -> impl Stream<Item = Event<Self::Item>> + Send + 'static;
    fn process_event(&self, item: Event<Self::Item>);
}

impl<T: TypedProducer> Producer for T {
    fn create_stream(&self) -> DynStream {
        Box::pin(TypedProducer::event_stream(self).map(
            |Event {
                 timestamp,
                 tie_break,
                 obj,
             }| {
                Event {
                    timestamp,
                    tie_break,
                    obj: Box::new(obj) as _,
                }
            },
        ))
    }

    fn process_event(
        &self,
        Event {
            timestamp,
            tie_break,
            obj,
        }: Event<Box<dyn Any>>,
    ) {
        let event = Event {
            timestamp,
            tie_break,
            obj: *obj.downcast().unwrap(),
        };
        TypedProducer::process_event(self, event);
    }
}

type DynStream = Pin<Box<dyn Send + Stream<Item = Event<Box<dyn Any + Send>>>>>;

pub trait Producer: private::Sealed {
    fn create_stream(&self) -> DynStream;
    fn process_event(&self, item: Event<Box<dyn Any>>);
}

struct HeapEntry {
    next_event: Event<Box<dyn Any + Send>>,
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

async fn background_task(producers: Vec<Accessor<dyn Producer>>) {
    let mut heap = BinaryHeap::with_capacity(producers.len());

    let futures: Vec<_> = producers
        .into_iter()
        .map(|mut acc| {
            let (tx, rx) = oneshot::channel();
            acc.send(|_, p| {
                tx.send(p.create_stream())
                    .unwrap_or_else(|_| panic!("Could not send"))
            })
            .unwrap();
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
        mut acc,
        mut stream,
    }) = heap.pop()
    {
        let Event {
            timestamp,
            tie_break,
            obj,
        } = next_event;
        acc.send(move |_, p| {
            p.process_event(Event {
                timestamp,
                tie_break,
                obj: obj as _,
            })
        })
        .unwrap();

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
