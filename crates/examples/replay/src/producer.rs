use common::dytor::MainArgs;
use tokio_stream::{Stream, StreamExt};

use crate::{DynStream, Event, TokioSingleThread, UntypedBox};

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

#[allow(private_interfaces)]
pub trait Producer: private::Sealed {
    fn create_stream(&mut self, args: &mut MainArgs, runtime: TokioSingleThread) -> DynStream;
    fn process_event(&mut self, args: &mut MainArgs, item: Event<UntypedBox>);
}
