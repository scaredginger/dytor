use core::sync::atomic::AtomicPtr;

use __private::ListNode;

use crate::{Actor, ActorMeta};
use std::collections::HashMap;
use std::sync::Arc;

static INIT_FNS: AtomicPtr<ListNode> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Default)]
pub struct Registry {
    pub actors: HashMap<&'static str, ActorMeta>,
}

impl Registry {
    pub fn load() -> Arc<Self> {
        use std::sync::atomic::Ordering;

        let mut registry = Registry::default();
        let mut ptr = INIT_FNS.load(Ordering::Acquire);
        while ptr != core::ptr::null_mut() {
            let node = unsafe { &*ptr };
            (node.f)(&mut registry).unwrap();
            ptr = node.next.load(Ordering::Relaxed);
        }
        Arc::new(registry)
    }
}

#[macro_export]
macro_rules! register_actor {
    ($struct:ident) => {
        register_actor!($struct, __register_actor_private);
    };
    ($struct:ident, $ns:ident) => {
        mod $ns {
            use super::$struct;
            use std::sync::atomic::Ordering;
            use $crate::registry::__private::{ctor, init_fn, init_node, ListNode};

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

    let prev = registry.actors.insert(name, ActorMeta::new::<T>());
    if let Some(_) = prev {
        anyhow::bail!("Multiple actors registered for {name}")
    }
    Ok(())
}

pub mod __private {
    use super::Registry;
    use core::{
        ptr,
        sync::atomic::{AtomicPtr, Ordering},
    };
    pub use ctor;

    use crate::Actor;

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
