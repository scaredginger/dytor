use std::alloc::Layout;
use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::mem::{align_of, MaybeUninit};

use serde::de::{Deserialize, DeserializeOwned};

use crate::{Dyn, InitStage};

pub use rian_proc_macros::{uniquely_named, UniquelyNamed};

pub trait UniquelyNamed {
    fn name() -> &'static str;
}

pub trait Actor: Any + Unpin + Sized + UniquelyNamed {
    type Config: Debug + DeserializeOwned;

    fn instantiate(data: &InitStage, config: Self::Config) -> anyhow::Result<Self>;
}

#[derive(Clone, Copy, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct TraitId(TypeId);

impl TraitId {
    pub(crate) const fn of<T: ?Sized + Dyn>() -> Self {
        TraitId(TypeId::of::<T>())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ActorVTable {
    pub(crate) deserialize_yaml: fn(serde_yaml::Deserializer) -> anyhow::Result<Box<dyn Any>>,
    pub(crate) deserialize_yaml_value: fn(&serde_yaml::Value) -> anyhow::Result<Box<dyn Any>>,
    pub(crate) constructor:
        fn(&InitStage, dest: &mut [u8], config: Box<dyn Any>) -> anyhow::Result<()>,
    pub(crate) drop: fn(&mut [u8]),
    pub(crate) name: fn() -> &'static str,
    pub(crate) type_id: TypeId,
    size: usize,
    align: usize,
}

impl ActorVTable {
    pub(crate) fn layout(&self) -> Layout {
        Layout::from_size_align(self.size, self.align).unwrap()
    }

    pub(crate) const fn new<T: Actor>() -> Self {
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
                assert!(dest.len() <= std::mem::size_of::<T>());
                let config: Box<T::Config> = config.downcast().unwrap();
                let dest: *mut MaybeUninit<T> = dest.as_mut_ptr().cast();
                assert_eq!(dest.align_offset(align_of::<T>()), 0);

                let res = T::instantiate(init_data, *config)?;
                unsafe { &mut *dest }.write(res);
                Ok(())
            },
            drop: |buf| {
                assert!(buf.len() <= std::mem::size_of::<T>());
                let this = buf.as_mut_ptr().cast::<T>();
                assert_eq!(this.align_offset(align_of::<T>()), 0);
                unsafe { std::ptr::drop_in_place(this) }
            },
            name: T::name,
            type_id: TypeId::of::<T>(),

            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
        }
    }
}
