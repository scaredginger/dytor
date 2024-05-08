use std::pin::Pin;

use common::chrono::{DateTime, Utc};
pub use tokio;

mod synchronizer;
mod tokio_wrapper;
pub use tokio_stream;
use tokio_stream::Stream;
pub use tokio_wrapper::{LazyDynFut, TokioSingleThread};
mod producer;

pub use producer::{Producer, TypedProducer};
pub use synchronizer::Synchronizer;

type DynStream = Pin<Box<dyn Send + Stream<Item = Event<UntypedBox>>>>;

#[repr(transparent)]
struct UntypedBox(*mut u8);
unsafe impl Send for UntypedBox {}

#[derive(Clone, Copy)]
pub struct Event<T> {
    pub timestamp: DateTime<Utc>,
    pub tie_break: u64,
    pub obj: T,
}
