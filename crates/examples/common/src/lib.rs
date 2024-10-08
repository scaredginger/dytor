// re-export common libs here so they get linked into a single shared lib
pub use anyhow;
pub use chrono;
pub use itertools;
pub use serde;
pub use serde_value;

pub use dytor;

pub trait CommonTrait: 'static {
    fn print_self(&self);
}

impl<T: 'static + std::fmt::Debug> CommonTrait for T {
    fn print_self(&self) {
        println!("{self:?}");
    }
}
