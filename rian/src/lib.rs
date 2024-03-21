use __private::Registry;
use serde::Deserialize;
use std::{
    any::Any, collections::HashMap, fmt::Debug, future::Future, marker::PhantomData, pin::Pin,
};

#[cfg(feature = "main")]
pub mod app;

#[cfg(feature = "main")]
pub use app::main;

#[cfg(feature = "lib")]
pub mod lib_util;

pub trait Actor: Any + Unpin + Sized {
    type Config<'de>: Debug + Deserialize<'de>;

    fn instantiate(data: &InitData) -> anyhow::Result<Self>;
    fn name() -> &'static str;
    fn run(&self, data: &MainData) -> impl Future<Output = anyhow::Result<()>> + Send + Sync;

    fn terminate(&self) -> impl Future<Output = ()> + Send + Sync {
        async {}
    }
}

pub struct InitData(Registry);

#[derive(Clone, Copy)]
pub struct Ref<T: ?Sized> {
    offset_ptr: u32,
    _phantom: PhantomData<*const T>,
}

impl<T: ?Sized> Ref<T> {
    pub fn get(self, data: &MainData) -> &T {
        data.get(self)
    }
}

impl InitData {
    pub fn request<T: ?Sized>(&self) -> Ref<T> {
        unimplemented!();
    }
}

pub struct MainData(Registry);

impl MainData {
    pub fn get<T: ?Sized>(&self, _r: Ref<T>) -> &T {
        unimplemented!();
    }
}

#[derive(Clone, Copy)]
struct ActorTypeInfo {
    constructor: fn(&InitData, dest: &mut [u8]) -> anyhow::Result<()>,
    run: for<'a> fn(
        *const u8,
        &'a MainData,
    ) -> Pin<Box<dyn 'a + Future<Output = anyhow::Result<()>> + Send + Sync>>,
    terminate: fn(*const u8) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
    drop: fn(*mut u8),
    size: usize,
    align: usize,
}

pub mod __private {
    use std::collections::HashMap;

    use crate::ActorTypeInfo;

    #[derive(Default)]
    pub struct Registry {
        pub(crate) actors: HashMap<&'static str, ActorTypeInfo>,
    }
}
