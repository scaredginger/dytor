// re-export common libs here so they get linked into a single shared lib
pub use anyhow;
pub use rian;
pub use serde_yaml;
pub use tokio;

pub trait CommonTrait: 'static {
    fn print_self(&self);
}

impl<T: 'static + std::fmt::Debug> CommonTrait for T {
    fn print_self(&self) {
        println!("{self:?}");
    }
}
