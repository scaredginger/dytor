#![feature(ptr_metadata, const_type_id)]
#![allow(private_bounds)]

use std::ptr::{DynMetadata, Pointee};

pub use context::{ContextId, InitStage, MainStage};

pub use paste;

mod actor;
pub mod lookup;
pub mod queue;
pub use actor::{uniquely_named, Actor, UniquelyNamed};
mod arena;
pub mod config;
pub use config::Config;
pub mod registry;
pub(crate) use registry::Registry;
pub mod app;
mod context;
mod runtime;

pub(crate) trait Dyn: 'static + Pointee<Metadata = DynMetadata<Self>> {}
impl<T: ?Sized + 'static + Pointee<Metadata = DynMetadata<T>>> Dyn for T {}
