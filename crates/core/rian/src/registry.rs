use core::sync::atomic::AtomicPtr;
use std::any::{type_name, Any, TypeId};
use std::mem::MaybeUninit;
use std::ptr::DynMetadata;
use std::sync::Arc;

pub(crate) use __private::ActorRegistered;
pub(crate) use __private::InterfaceMetadata;
use __private::ListNode;

use crate::object::actor;
use crate::{
    object::{TraitId, VTable},
    Actor,
};
use std::collections::HashMap;

static INIT_FNS: AtomicPtr<ListNode> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Default)]
pub struct RegistryBuilder {
    pub(crate) actor_types: HashMap<TypeId, VTable>,
    pub(crate) trait_types: HashMap<TraitId, Vec<InterfaceMetadata>>,
    pub(crate) name_to_type_id: HashMap<&'static str, TypeId>,
    pub(crate) resource_constructors:
        HashMap<TypeId, Arc<dyn Send + Sync + Fn() -> Box<dyn Any + Send + Sync>>>,
}

pub(crate) struct Registry {
    pub(crate) actor_types: HashMap<TypeId, VTable>,
    pub(crate) trait_types: HashMap<TraitId, Box<[InterfaceMetadata]>>,
    pub(crate) name_to_type_id: HashMap<&'static str, TypeId>,
    pub(crate) resource_constructors:
        HashMap<TypeId, Arc<dyn Send + Sync + Fn() -> Box<dyn Any + Send + Sync>>>,
}

impl Registry {
    pub(crate) fn get() -> &'static Self {
        use std::sync::atomic::Ordering;
        use std::sync::OnceLock;

        static REGISTRY: OnceLock<Registry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let mut registry = RegistryBuilder::default();
            let mut ptr = INIT_FNS.load(Ordering::Acquire);
            while !ptr.is_null() {
                let node = unsafe { &*ptr };
                (node.f)(&mut registry).unwrap();
                ptr = node.next.load(Ordering::Relaxed);
            }

            let RegistryBuilder {
                actor_types,
                name_to_type_id,
                trait_types,
                resource_constructors,
            } = registry;

            Registry {
                actor_types,
                trait_types: trait_types
                    .into_iter()
                    .map(|(k, v)| (k, v.into_boxed_slice()))
                    .collect(),
                name_to_type_id,
                resource_constructors,
            }
        })
    }

    pub(crate) fn by_name(&self, name: &str) -> Option<(TypeId, &VTable)> {
        let type_id = self.name_to_type_id.get(name)?;
        let vtable = self.actor_types.get(type_id)?;
        Some((*type_id, vtable))
    }
}

#[macro_export]
macro_rules! register_resource {
    ($closure:expr) => {
        $crate::paste::paste! {
            #[allow(non_snake_case)]
            mod __declare_resource {
                use $crate::registry::__private::*;
                use super::*;

                static NODE: ListNode = ListNode::new(|r| init_resource(r, $closure));

                #[ctor::ctor]
                fn foo() {
                    init_node(&NODE);
                }
            }
        }
    };
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
                use $crate::registry::__private::*;

                fn get_metadata() -> impl Iterator<Item = (TraitId, InterfaceMetadata)> {
                    [
                        $(
                            metadata_helper::<dyn $trait, $struct>(std::ptr::null::<$struct>())
                        ,)*
                    ].into_iter()
                }

                static NODE: ListNode = ListNode::new(|r| init_actor::<$struct>(r, get_metadata()));

                #[ctor::ctor]
                fn $struct() {
                    init_node(&NODE);
                }

                impl ActorRegistered for $struct {}
            }
        }
    };
}

// this Any is unimportant; I just need something I'm confident has the
// same size and align of any trait object metadata
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct DynMetaPlaceholder(MaybeUninit<DynMetadata<dyn Any>>);

pub mod __private {
    use super::*;
    pub use crate::object::TraitId;
    use crate::Dyn;
    use core::{ptr, sync::atomic::Ordering};
    pub use ctor;
    pub use libc;

    #[diagnostic::on_unimplemented(
        message = "The runtime will not recognise {Self} as an actor.",
        label = "`{Self}` has not been registered as an actor.",
        note = "Adding `register_actor!({Self});` will fix this error."
    )]
    pub trait ActorRegistered {}

    #[diagnostic::on_unimplemented(
        message = "The runtime will not recognise {Self} as a resource.",
        label = "`{Self}` has not been registered as a resource.",
        note = "Adding `register_resource!({Self});` will fix this error."
    )]
    pub trait ResourceRegistered {}

    pub fn metadata_helper<D: ?Sized + Dyn, S: 'static>(
        ptr: *const D,
    ) -> (TraitId, InterfaceMetadata) {
        let dyn_meta = std::ptr::metadata(ptr);
        (
            TraitId::of::<D>(),
            InterfaceMetadata {
                dyn_meta: unsafe {
                    std::mem::transmute::<
                        std::ptr::DynMetadata<D>,
                        crate::registry::DynMetaPlaceholder,
                    >(dyn_meta)
                },
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

    pub fn init_resource<T: 'static + Send + Sync>(
        registry: &mut RegistryBuilder,
        f: fn() -> T,
    ) -> anyhow::Result<()> {
        let prev = registry
            .resource_constructors
            .insert(TypeId::of::<T>(), Arc::new(move || Box::new(f()) as _));
        anyhow::ensure!(
            prev.is_none(),
            "Resource {} registered twice",
            type_name::<T>()
        );
        Ok(())
    }

    pub fn init_actor<T: Actor>(
        registry: &mut RegistryBuilder,
        traits: impl Iterator<Item = (TraitId, InterfaceMetadata)>,
    ) -> anyhow::Result<()> {
        let prev = registry
            .actor_types
            .insert(TypeId::of::<T>(), actor::create_vtable::<T>());
        for (trait_id, meta) in traits {
            registry.trait_types.entry(trait_id).or_default().push(meta);
        }
        anyhow::ensure!(
            prev.is_none(),
            "Actor {} registered twice",
            type_name::<T>()
        );
        assert!(registry
            .name_to_type_id
            .insert(T::name(), TypeId::of::<T>())
            .is_none());
        Ok(())
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
