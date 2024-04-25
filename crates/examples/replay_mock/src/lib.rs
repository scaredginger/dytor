use std::sync::Arc;
use std::time::Duration;

use replay::synchronizer::{Event, Producer, Stream, Synchronizer, TypedProducer};

use replay::tokio::sync::mpsc;
use replay::tokio_stream::wrappers::ReceiverStream;
use replay::{tokio, tokio_stream, TokioSingleThread};
use common::anyhow;
use common::chrono::{DateTime, TimeZone, Utc};
use common::rian::lookup::{BroadcastGroup, Key};
use common::rian::{register_actor, Accessor, Actor, InitArgs, MainArgs, UniquelyNamed};
use common::itertools::Itertools;

#[derive(UniquelyNamed)]
pub struct IntervalUnitProducer {
    times: Vec<DateTime<Utc>>,
    consumers: BroadcastGroup<IntervalUnitConsumer>,
    tokio: Key<TokioSingleThread>,
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

    fn init(args: InitArgs<Self>, config: ()) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    fn is_finished(&self) -> bool {
        true
    }

    fn stop(&mut self) {}
}

register_actor!(IntervalUnitProducer {
    dyn Producer,
});

impl Actor for IntervalUnitProducer {
    type Config = ();

    fn init(mut args: InitArgs<Self>, config: ()) -> anyhow::Result<Self> {
        let mut times = Vec::new();
        for i in 1..=10i64 {
            times.push(DateTime::from_timestamp_nanos(i * 1_000_000_000));
        }
        times.reverse();
        // let times = Utc::offset_from_utc_datetime(&self, utc)
        let consumers = args.query().broadcast_group();

        let tokio = args.query().exactly_one_key();
        Ok(Self {
            times,
            consumers,
            tokio,
        })
    }

    fn is_finished(&self) -> bool {
        true
    }

    fn stop(&mut self) {}
}

impl TypedProducer for IntervalUnitProducer {
    type Item = ();

    fn event_stream(
        &mut self,
        mut args: MainArgs,
    ) -> impl Stream<Item = Event<Self::Item>> + Send + 'static {
        let (tx, rx) = mpsc::channel(2);
        let mut times = self.times.clone();
        args.send_msg(self.tokio, move |_, tokio| {
            tokio.spawn_with(move || async move {
                while let Some(t) = times.pop() {
                    let ev = Event {
                        timestamp: t,
                        tie_break: 0,
                        obj: (),
                    };
                    tx.send(ev).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            });
        });
        ReceiverStream::from(rx)
    }

    fn process_event(&mut self, mut args: MainArgs, item: Event<Self::Item>) {
        args.broadcast(&self.consumers, move |args, c| {
            c.recv_event(&item);
        });
    }
}
