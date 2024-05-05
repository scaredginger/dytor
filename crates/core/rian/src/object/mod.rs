use std::alloc::Layout;
use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::mem::align_of;
use std::ptr::DynMetadata;

use serde::de::DeserializeOwned;

pub use rian_proc_macros::{uniquely_named, UniquelyNamed};

use self::actor::ActorConstructor;

pub(crate) mod actor;

pub trait UniquelyNamed {
    fn name() -> &'static str;
}

#[derive(Clone, Copy, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct TraitId(TypeId);

impl TraitId {
    pub(crate) const fn of<T: ?Sized + 'static>() -> Self {
        TraitId(TypeId::of::<DynMetadata<T>>())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ObjectConstructor {
    Actor(ActorConstructor),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct VTable {
    pub(crate) deserialize_yaml_value:
        fn(serde_value::Value) -> anyhow::Result<Box<dyn Any + Send>>,
    pub(crate) constructor: ObjectConstructor,
    pub(crate) drop: unsafe fn(*mut u8),
    pub(crate) name: fn() -> &'static str,
    pub(crate) type_id: TypeId,
    size: usize,
    align: usize,
}

impl VTable {
    pub(crate) fn layout(&self) -> Layout {
        Layout::from_size_align(self.size, self.align).unwrap()
    }

    const fn new_impl<T: Any + UniquelyNamed, Config: 'static + Debug + DeserializeOwned + Send>(
        constructor: ObjectConstructor,
    ) -> Self {
        Self {
            deserialize_yaml_value: |d| match Config::deserialize(d) {
                Ok(x) => Ok(Box::new(x) as _),
                Err(e) => anyhow::bail!("Could not deserialize config for {} {e:?}", T::name()),
            },
            drop: |ptr| {
                assert_eq!(ptr.align_offset(align_of::<T>()), 0);
                let this = ptr.cast::<T>();
                unsafe { std::ptr::drop_in_place(this) };
            },
            name: T::name,
            type_id: TypeId::of::<T>(),
            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
            constructor,
        }
    }
}
