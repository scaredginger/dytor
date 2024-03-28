#![feature(ptr_metadata, const_type_id)]

use actor::{ActorVTable, TraitId};
pub use paste;
use registry::DynMetaPlaceholder;
use serde::Deserialize;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    ptr::{DynMetadata, Pointee},
    sync::Arc,
};

mod actor;
pub use actor::{uniquely_named, Actor, UniquelyNamed};
mod arena;
pub mod config;
use arena::Arena;
pub use config::Config;
pub mod registry;
pub(crate) use registry::Registry;
pub mod app;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ActorId(pub(crate) u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct ContextId(u32);

type PhantomUnsend = PhantomData<*mut ()>;

pub(crate) trait Dyn: 'static + Pointee<Metadata = DynMetadata<Self>> {}
impl<T: ?Sized + 'static + Pointee<Metadata = DynMetadata<T>>> Dyn for T {}

pub struct Context {
    internal_messages: Vec<Box<dyn Fn(*mut u8)>>,
    _unsend_marker: PhantomUnsend,
}

impl Context {
    pub(crate) fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }
}

#[derive(Clone)]
struct ActorData {
    id: ActorId,
    vtable: &'static ActorVTable,
    loc: UntypedRef,
}

#[derive(Default)]
struct ActorTree {
    // TODO: actually make this a tree
    actors: Vec<ActorData>,
}

impl ActorTree {
    fn lookup<T: Actor>(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = Ref<T>> {
        let type_id = TypeId::of::<T>();
        self.actors
            .iter()
            .filter(move |actor| actor.vtable.type_id == type_id)
            .map(|actor| Ref {
                loc: actor.loc,
                _phantom: PhantomData::default(),
            })
    }

    fn lookup_dyn<T: ?Sized + Dyn>(
        &self,
        from_actor: ActorId,
    ) -> impl '_ + Iterator<Item = DynRef<T>> {
        let trait_id = TraitId::of::<T>();
        let types = match Registry::get().trait_types.get(&trait_id) {
            Some(types) => types.as_ref(),
            None => &[],
        };
        self.actors
            .iter()
            .filter_map(|actor| {
                Some((
                    actor,
                    types.iter().find(|x| x.type_id == actor.vtable.type_id)?,
                ))
            })
            .map(|(actor, t)| DynRef {
                dyn_metadata: t.dyn_meta,
                _phantom: PhantomData::default(),
                loc: actor.loc,
            })
    }
}

#[derive(Default)]
pub(crate) struct ContextBuilder {
    id: Option<ContextId>,
    actors: Option<Vec<ActorData>>,
    arena: Option<Arena>,
    tree: Option<Arc<ActorTree>>,
}

impl ContextBuilder {
    pub fn with_id(mut self, id: ContextId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn place_actors(&mut self, actors: Vec<(ActorId, &'static ActorVTable)>) -> &[ActorData] {
        let (arena, offsets) = Arena::from_layouts(&Vec::from_iter(
            actors.iter().map(|(_, vtable)| vtable.layout()),
        ));
        self.arena = Some(arena);
        self.actors = Some(
            offsets
                .into_iter()
                .zip(actors)
                .map(|(offset, (id, vtable))| ActorData {
                    id,
                    vtable,
                    loc: UntypedRef {
                        context_id: self.id.unwrap(),
                        offset_ptr: offset.try_into().unwrap(),
                    },
                })
                .collect(),
        );
        self.actors.as_ref().unwrap()
    }

    pub fn set_tree(&mut self, tree: Arc<ActorTree>) {
        self.tree = Some(tree)
    }

    pub fn build(self, mut actor_configs: HashMap<ActorId, Box<dyn Any>>) -> Context {
        let mut arena = self.arena.unwrap();
        let mut context = Context {
            internal_messages: Vec::new(),
            _unsend_marker: Default::default(),
        };

        let tree = self.tree.unwrap();
        for actor in self.actors.unwrap() {
            let init_stage = InitStage {
                context: &mut context,
                tree: &tree,
                actor_being_constructed: actor.id,
            };
            let offset = actor.loc.offset_ptr;
            let buf = arena.at_offset(offset as usize, actor.vtable.layout());

            let config = actor_configs.remove(&actor.id).unwrap();
            (actor.vtable.constructor)(&init_stage, buf, config);
        }
        assert!(
            actor_configs.is_empty(),
            "Actors not constructed: {:?}",
            actor_configs.keys()
        );

        context
    }
}

pub struct InitStage<'init> {
    pub(crate) context: &'init mut Context,
    pub(crate) tree: &'init ActorTree,
    pub(crate) actor_being_constructed: ActorId,
}

#[derive(Clone, Copy)]
struct UntypedRef {
    context_id: ContextId,
    offset_ptr: u32,
}

#[derive(Clone, Copy)]
pub struct DynRef<T: ?Sized> {
    dyn_metadata: DynMetaPlaceholder,
    loc: UntypedRef,
    _phantom: PhantomData<*const T>,
}

#[derive(Clone, Copy)]
pub struct Ref<T: ?Sized> {
    loc: UntypedRef,
    _phantom: PhantomData<*const T>,
}

impl<T: ?Sized> Ref<T> {
    fn from_loc(loc: UntypedRef) -> Self {
        Self {
            loc,
            _phantom: PhantomData::default(),
        }
    }
}

impl<'a> InitStage<'a> {
    pub fn request<T: Actor>(&self) -> impl '_ + Iterator<Item = Ref<T>> {
        self.tree.lookup::<T>(self.actor_being_constructed)
    }
    pub fn request_dyn<T: ?Sized + Dyn>(&self) -> impl '_ + Iterator<Item = DynRef<T>> {
        self.tree.lookup_dyn::<T>(self.actor_being_constructed)
    }
}

pub struct MainStage {
    pub(crate) context: Context,
    pub(crate) actor_running: ActorId,
}
