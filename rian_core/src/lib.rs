use serde::{de::DeserializeOwned, Deserialize};
use std::{
    alloc::Layout, any::Any, fmt::Debug, future::Future, marker::PhantomData, mem::MaybeUninit,
    pin::Pin, sync::Arc,
};

pub mod registry;
pub use registry::Registry;

pub trait Actor: Any + Unpin + Sized {
    type Config: Debug + DeserializeOwned;

    fn instantiate(data: &InitData, config: Self::Config) -> anyhow::Result<Self>;
    fn name() -> &'static str;
    fn run(&self, data: &MainData) -> impl Future<Output = anyhow::Result<()>>;

    fn terminate(&self) -> impl Future<Output = ()> {
        async {}
    }
}

pub struct Context {
    registry: std::sync::Arc<Registry>,
}

impl Context {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }
}

pub struct InitData(Context);

impl From<Context> for InitData {
    fn from(context: Context) -> Self {
        Self(context)
    }
}

#[derive(Clone, Copy)]
pub struct Ref<T: ?Sized> {
    offset_ptr: u32,
    _phantom: PhantomData<*const T>,
}

impl InitData {
    pub fn request<T: ?Sized>(&self) -> Ref<T> {
        unimplemented!();
    }
}

pub struct MainData(Context);

impl From<InitData> for MainData {
    fn from(data: InitData) -> Self {
        Self(data.0)
    }
}

impl MainData {
    pub fn send<T: ?Sized>(&self, _r: Ref<T>, msg: impl FnOnce(&T) + Send) {
        unimplemented!();
    }
}

#[derive(Clone, Copy)]
pub struct ActorMeta {
    pub deserialize_yaml: fn(serde_yaml::Deserializer) -> anyhow::Result<Box<dyn Any>>,
    pub deserialize_yaml_value: fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Any>>,
    pub constructor: fn(&InitData, dest: &mut [u8], config: Box<dyn Any>) -> anyhow::Result<()>,
    pub run: for<'a> fn(
        *const u8,
        &'a MainData,
    ) -> Pin<Box<dyn 'a + Future<Output = anyhow::Result<()>>>>,
    pub terminate: fn(*const u8) -> Pin<Box<dyn Future<Output = ()>>>,
    pub drop: fn(*mut u8),
    size: usize,
    align: usize,
}

impl ActorMeta {
    pub fn layout(&self) -> Layout {
        Layout::from_size_align(self.size, self.align).unwrap()
    }

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
