use core::sync::atomic::AtomicPtr;
use std::{any::TypeId, mem::MaybeUninit};

use crate::{Actor, ActorTypeInfo, Registry};

static INIT_FNS: AtomicPtr<ListNode> = AtomicPtr::new(core::ptr::null_mut());

#[macro_export]
macro_rules! declare_rian_lib {
    () => {
        use std::sync::atomic::Ordering;
        use $crate::__private::Registry;

        #[export_name = "load_rian_module"]
        extern "C" fn load_rian_module(registry: &mut Registry) {
            $crate::lib_util::__private::do_load(registry);
        }
    };
}

#[macro_export]
macro_rules! register_actor {
    ($struct:ident, $ns:ident) => {
        mod $ns {
            use super::$struct;
            use std::sync::atomic::Ordering;
            use $crate::lib_util::__private::{ctor, init_fn, init_node, ListNode};

            static NODE: ListNode = ListNode::new(init_fn::<$struct>);

            #[ctor::ctor]
            fn init() {
                init_node(&NODE);
            }
        }
    };
}

fn init_fn<T: Actor>(registry: &mut Registry) -> anyhow::Result<()> {
    let name = T::name();

    let prev = registry.actors.insert(
        name,
        ActorTypeInfo {
            constructor: |init_data, dest| {
                assert!(dest.len() >= std::mem::size_of::<T>());
                let dest: *mut MaybeUninit<T> = dest.as_mut_ptr().cast();
                assert!(dest as usize % std::mem::align_of::<T>() == 0);

                let res = T::instantiate(init_data)?;
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
        },
    );
    if let Some(_) = prev {
        anyhow::bail!("Multiple actors registered for {name}")
    }
    Ok(())
}

pub mod __private {
    use core::{
        ptr,
        sync::atomic::{AtomicPtr, Ordering},
    };
    pub use ctor;

    use crate::{Actor, __private::Registry};

    pub fn do_load(registry: &mut Registry) {
        let mut ptr = super::INIT_FNS.load(Ordering::Acquire);
        while ptr != core::ptr::null_mut() {
            let node = unsafe { &*ptr };
            (node.f)(registry).unwrap();
            ptr = node.next.load(Ordering::Relaxed);
        }
    }
    pub struct ListNode {
        pub(super) f: fn(r: &mut Registry) -> anyhow::Result<()>,
        pub(super) next: AtomicPtr<ListNode>,
    }

    impl ListNode {
        pub const fn new(f: fn(&mut Registry) -> anyhow::Result<()>) -> Self {
            Self {
                f,
                next: AtomicPtr::new(ptr::null_mut()),
            }
        }
    }

    pub fn init_fn<T: Actor>(registry: &mut Registry) -> anyhow::Result<()> {
        super::init_fn::<T>(registry)
    }

    pub fn init_node(node: &ListNode) {
        let node_addr = (node as *const ListNode).cast_mut();

        let mut prev_init = super::INIT_FNS.load(Ordering::Relaxed);
        loop {
            node.next.store(prev_init, Ordering::Relaxed);
            match super::INIT_FNS.compare_exchange_weak(
                prev_init,
                node_addr,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => prev_init = x,
            }
        }
    }
}

use __private::ListNode;
