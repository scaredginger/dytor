#![feature(ptr_metadata, const_type_id, lazy_cell)]
#![allow(private_bounds)]

use std::ptr::{DynMetadata, Pointee};

pub use context::{ContextId, InitArgs, MainArgs};

pub use paste;

pub mod lookup;
mod object;
pub mod queue;
pub use object::{actor::Actor, UniquelyNamed};
mod arena;
pub mod config;
pub use config::Config;
pub mod registry;
pub(crate) use registry::Registry;
mod context;
mod runtime;

pub use context::Accessor;
pub use runtime::run;

pub(crate) trait Dyn: 'static + Pointee<Metadata = DynMetadata<Self>> {}
impl<T: ?Sized + 'static + Pointee<Metadata = DynMetadata<T>>> Dyn for T {}
