use std::time::Duration;

use replay::synchronizer::{Event, Producer, Stream, TypedProducer};

use replay::tokio::sync::mpsc;
use replay::tokio_stream::wrappers::ReceiverStream;
use replay::{tokio, TokioSingleThread};
use common::anyhow;
use common::chrono::DateTime;
use common::rian::lookup::BroadcastGroup;
use common::rian::{register_actor, Actor, InitArgs, MainArgs, UniquelyNamed};

#[derive(UniquelyNamed)]
pub struct IntervalUnitProducer {
    consumers: BroadcastGroup<IntervalUnitConsumer>,
    rx: Option<tokio::sync::mpsc::Receiver<Event<()>>>,
}

#[derive(UniquelyNamed)]
pub struct IntervalUnitConsumer {}

impl IntervalUnitConsumer {
    fn recv_event(&self, ev: &Event<()>) {
        println!("Received event at {}", ev.timestamp);
    }
}

register_actor!(IntervalUnitConsumer);

impl Actor for IntervalUnitConsumer {
    type Config = ();

    fn init(_args: InitArgs<Self>, _config: ()) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

register_actor!(IntervalUnitProducer {
    dyn Producer,
});

impl Actor for IntervalUnitProducer {
    type Config = ();

    fn init(mut args: InitArgs<Self>, _config: ()) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel(2);
        let tokio = args.get_resource::<TokioSingleThread>();
        tokio.spawn_with(move || async move {
            for t in 1..=10i64 {
                let t = DateTime::from_timestamp_nanos(t * 1_000_000_000);
                let ev = Event {
                    timestamp: t,
                    tie_break: 0,
                    obj: (),
                };
                tx.send(ev).await.unwrap();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        Ok(Self {
            consumers: args.query().broadcast_group(),
            rx: Some(rx),
        })
    }
}

impl TypedProducer for IntervalUnitProducer {
    type Item = ();

    fn event_stream(
        &mut self,
        _args: &mut MainArgs,
    ) -> impl Stream<Item = Event<Self::Item>> + Send + 'static {
        ReceiverStream::from(self.rx.take().unwrap())
    }

    fn process_event(&mut self, args: &mut MainArgs, item: Event<Self::Item>) {
        args.broadcast(&self.consumers, move |_, c| {
            c.recv_event(&item);
        });
    }
}
