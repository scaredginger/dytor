// re-export common libs here so they get linked into a single shared lib
pub use rian_core as core;
// pub use serde;
pub use serde_yaml;

pub use rian_core::*;

pub trait CommonTrait {}
