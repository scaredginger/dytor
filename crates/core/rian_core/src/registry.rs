use core::sync::atomic::AtomicPtr;
use std::any::{type_name, Any, TypeId};
use std::mem::MaybeUninit;
use std::ptr::{DynMetadata};

pub(crate) use __private::InterfaceMetadata;
use __private::ListNode;

use crate::actor::{Actor, ActorVTable, TraitId};
use std::collections::HashMap;

static INIT_FNS: AtomicPtr<ListNode> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Default)]
pub struct RegistryBuilder {
    pub(crate) actor_types: HashMap<TypeId, ActorVTable>,
    pub(crate) trait_types: HashMap<TraitId, Vec<InterfaceMetadata>>,
}

pub(crate) struct Registry {
    pub(crate) actor_types: HashMap<TypeId, ActorVTable>,
    pub(crate) trait_types: HashMap<TraitId, Box<[InterfaceMetadata]>>,
}

impl Registry {
    pub fn get() -> &'static Self {
        use std::sync::atomic::Ordering;
        use std::sync::OnceLock;

        static REGISTRY: OnceLock<Registry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let mut registry = RegistryBuilder::default();
            let mut ptr = INIT_FNS.load(Ordering::Acquire);
            while ptr != core::ptr::null_mut() {
                let node = unsafe { &*ptr };
                (node.f)(&mut registry).unwrap();
                ptr = node.next.load(Ordering::Relaxed);
            }

            let RegistryBuilder {
                actor_types,
                trait_types,
            } = registry;
            Registry {
                actor_types,
                trait_types: trait_types
                    .into_iter()
                    .map(|(k, v)| (k, v.into_boxed_slice()))
                    .collect(),
            }
        })
    }
}

#[macro_export]
macro_rules! register_actor {
    ($struct:ident) => {
        register_actor!($struct {});
    };
    ($struct:ident { $(dyn $trait:ident),* $(,)? }) => {
        $crate::paste::paste! {
            #[allow(non_snake_case)]
            mod [<__declare_actor_ $struct>] {
                use super::{$struct, $($trait,)*};
                use std::{sync::atomic::Ordering, collections::HashMap, any::TypeId};
                use $crate::registry::__private::*;

                fn get_metadata() -> impl Iterator<Item = (TraitId, InterfaceMetadata)> {
                    [
                        $(
                            metadata_helper::<dyn $trait, $struct>(std::ptr::null::<$struct>())
                        ,)*
                    ].into_iter()
                }

                static NODE: ListNode = ListNode::new(|r| init_fn::<$struct>(r, get_metadata()));

                #[ctor::ctor]
                fn init() {
                    init_node(&NODE);
                }
            }
        }
    };
}

fn init_fn<T: Actor>(
    registry: &mut RegistryBuilder,
    traits: impl Iterator<Item = (TraitId, InterfaceMetadata)>,
) -> anyhow::Result<()> {
    let prev = registry
        .actor_types
        .insert(TypeId::of::<T>(), ActorVTable::new::<T>());
    for (trait_id, meta) in traits {
        registry.trait_types.entry(trait_id).or_default().push(meta);
    }
    if prev.is_some() {
        anyhow::bail!("Actor {} registered twice", type_name::<T>());
    }
    Ok(())
}

// this Any is unimportant; I just need something I'm confident has the
// same size and align of any trait object metadata
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct DynMetaPlaceholder(MaybeUninit<DynMetadata<dyn Any>>);

pub mod __private {
    use super::*;
    pub use crate::actor::TraitId;
    use crate::Dyn;
    use core::{ptr, sync::atomic::Ordering};
    pub use ctor;

    pub fn metadata_helper<D: ?Sized + Dyn, S: 'static>(
        ptr: *const D,
    ) -> (TraitId, InterfaceMetadata) {
        let dyn_meta = std::ptr::metadata(ptr);
        (
            TraitId::of::<D>(),
            InterfaceMetadata {
                dyn_meta: unsafe { std::mem::transmute(dyn_meta) },
                type_id: TypeId::of::<S>(),
            },
        )
    }

    pub struct InterfaceMetadata {
        pub(crate) type_id: TypeId,
        pub(crate) dyn_meta: DynMetaPlaceholder,
    }

    pub struct ListNode {
        pub(super) f: fn(r: &mut RegistryBuilder) -> anyhow::Result<()>,
        pub(super) next: AtomicPtr<ListNode>,
    }

    impl ListNode {
        pub const fn new(f: fn(&mut RegistryBuilder) -> anyhow::Result<()>) -> Self {
            Self {
                f,
                next: AtomicPtr::new(ptr::null_mut()),
            }
        }
    }

    pub fn init_fn<T: Actor>(
        registry: &mut RegistryBuilder,
        traits: impl Iterator<Item = (TraitId, InterfaceMetadata)>,
    ) -> anyhow::Result<()> {
        super::init_fn::<T>(registry, traits)
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
