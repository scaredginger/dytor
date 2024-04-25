use std::{
    any::Any,
    fmt::Debug,
    mem::{align_of, MaybeUninit},
};

use serde::de::DeserializeOwned;

use crate::{InitArgs, UniquelyNamed};

use super::{ObjectConstructor, VTable};

pub trait Actor: Any + Unpin + Sized + UniquelyNamed {
    type Config: Debug + DeserializeOwned + Send;

    fn init(args: InitArgs<Self>, config: Self::Config) -> anyhow::Result<Self>;
    fn is_finished(&self) -> bool;

    fn stop(&mut self);
}

pub(crate) type ActorConstructor = for<'a, 'b> unsafe fn(
    InitArgs<'a, ()>,
    dest: &'b mut [u8],
    config: Box<dyn Any>,
) -> anyhow::Result<()>;

pub(crate) fn create_vtable<T: Actor>() -> VTable {
    let constructor = ObjectConstructor::Actor(|args, dest, config| {
        assert_eq!(dest.len(), std::mem::size_of::<T>());
        let config: Box<T::Config> = config.downcast().unwrap();
        let dest: *mut MaybeUninit<T> = dest.as_mut_ptr().cast();
        assert_eq!(dest.align_offset(align_of::<T>()), 0);

        let args = unsafe { std::mem::transmute(args) };

        let res = T::init(args, *config)?;
        unsafe { &mut *dest }.write(res);
        Ok(())
    });

    let is_finished = |dest| {
        let dest = dest as *const T;
        unsafe { &*dest }.is_finished()
    };
    let stop = |dest| {
        let dest = dest as *mut T;
        unsafe { &mut *dest }.stop()
    };
    VTable::new_impl::<T, T::Config>(constructor, is_finished, stop)
}
