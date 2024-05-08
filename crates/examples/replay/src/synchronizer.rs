use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::pin::Pin;

use common::anyhow::Result;
use common::chrono::{DateTime, Utc};
use common::rian::{register_actor, Accessor, Actor, InitArgs, MainArgs, UniquelyNamed};
use tokio::sync::oneshot;
pub use tokio_stream::{Stream, StreamExt};

use crate::TokioSingleThread;

#[derive(Clone, Copy)]
pub struct Event<T> {
    pub timestamp: DateTime<Utc>,
    pub tie_break: u64,
    pub obj: T,
}

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

mod private {
    use super::*;
    pub trait Sealed {}
    impl<T: TypedProducer> Sealed for T {}
}

pub trait TypedProducer {
    type Item: Send + 'static;

    fn event_stream(
        &mut self,
        args: &mut MainArgs,
        runtime: TokioSingleThread,
    ) -> impl Stream<Item = Event<Self::Item>> + Send + 'static;
    fn process_event(&mut self, args: &mut MainArgs, item: Event<Self::Item>);
}

#[allow(private_interfaces)]
impl<T: TypedProducer> Producer for T {
    fn create_stream(&mut self, args: &mut MainArgs, runtime: TokioSingleThread) -> DynStream {
        Box::pin(TypedProducer::event_stream(self, args, runtime).map(
            |Event {
                 timestamp,
                 tie_break,
                 obj,
             }| Event {
                timestamp,
                tie_break,
                obj: UntypedBox(Box::leak(Box::new(obj)) as *mut _ as _),
            },
        ))
    }

    fn process_event(&mut self, args: &mut MainArgs, event: Event<UntypedBox>) {
        let Event {
            timestamp,
            tie_break,
            obj,
        } = event;
        let obj = *unsafe { Box::from_raw(obj.0 as *mut T::Item) };
        TypedProducer::process_event(
            self,
            args,
            Event {
                timestamp,
                tie_break,
                obj,
            },
        );
    }
}

type DynStream = Pin<Box<dyn Send + Stream<Item = Event<UntypedBox>>>>;

#[repr(transparent)]
struct UntypedBox(*mut u8);
unsafe impl Send for UntypedBox {}

#[allow(private_interfaces)]
pub trait Producer: private::Sealed {
    fn create_stream(&mut self, args: &mut MainArgs, runtime: TokioSingleThread) -> DynStream;
    fn process_event(&mut self, args: &mut MainArgs, item: Event<UntypedBox>);
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
