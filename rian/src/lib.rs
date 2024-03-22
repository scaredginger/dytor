use __private::Registry;
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    any::Any, collections::HashMap, fmt::Debug, future::Future, marker::PhantomData,
    mem::MaybeUninit, pin::Pin,
};

#[cfg(feature = "main")]
pub mod app;

#[cfg(feature = "main")]
pub use app::main;

#[cfg(feature = "lib")]
pub mod lib_util;

pub trait Actor: Any + Unpin + Sized {
    type Config: Debug + DeserializeOwned;

    fn instantiate(data: &InitData, config: Self::Config) -> anyhow::Result<Self>;
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
struct ActorVTable {
    deserialize_yaml: fn(serde_yaml::Deserializer) -> anyhow::Result<Box<dyn Any>>,
    deserialize_yaml_value: fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Any>>,
    constructor: fn(&InitData, dest: &mut [u8], config: Box<dyn Any>) -> anyhow::Result<()>,
    run: for<'a> fn(
        *const u8,
        &'a MainData,
    ) -> Pin<Box<dyn 'a + Future<Output = anyhow::Result<()>> + Send + Sync>>,
    terminate: fn(*const u8) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
    drop: fn(*mut u8),
    size: usize,
    align: usize,
}

impl ActorVTable {
    const fn new<T: Actor>() -> Self {
        Self {
            deserialize_yaml: |d| match T::Config::deserialize(d) {
                Ok(x) => Ok(Box::new(x) as Box<dyn Any>),
                Err(e) => anyhow::bail!("Could not deserialize config for {} {e:?}", T::name()),
            },
            deserialize_yaml_value: |d| match T::Config::deserialize(d) {
                Ok(x) => Ok(Box::new(x) as Box<dyn Any>),
                Err(e) => anyhow::bail!("Could not deserialize config for {} {e:?}", T::name()),
            },
            constructor: |init_data, dest, config| {
                let config: Box<T::Config> = config.downcast().unwrap();
                assert!(dest.len() >= std::mem::size_of::<T>());
                let dest: *mut MaybeUninit<T> = dest.as_mut_ptr().cast();
                assert!(dest as usize % std::mem::align_of::<T>() == 0);

                let res = T::instantiate(init_data, *config)?;
                unsafe { &mut *dest }.write(res);
                Ok(())
            },
            run: |this, data| {
                let this = this.cast::<T>();
                let this = unsafe { &*this };
                Box::pin(this.run(data))
            },
            terminate: |this| {
                let this = this.cast::<T>();
                let this = unsafe { &*this };
                Box::pin(this.terminate())
            },
            drop: |this| {
                let this = this.cast::<T>();
                unsafe { std::ptr::drop_in_place(this) }
            },
            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
        }
    }
}

pub mod __private {
    use std::collections::HashMap;

    use crate::ActorVTable;

    #[derive(Default)]
    pub struct Registry {
        pub(crate) actors: HashMap<&'static str, ActorVTable>,
    }
}
