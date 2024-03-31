#![feature(ptr_metadata, const_type_id)]

use actor::ActorVTable;
use lookup::{ActorTree, BroadcastGroup, Key, Loc, Query};
pub use paste;
use queue::{local::LocalQueue, Tx};
use serde::Deserialize;
use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    ptr::{DynMetadata, Pointee},
    sync::Arc,
};

mod actor;
mod lookup;
pub mod queue;
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
pub struct ContextId(u32);

impl ContextId {
    #[must_use]
    pub fn from_u32(x: u32) -> Self {
        Self(x)
    }
}

type PhantomUnsend = PhantomData<*mut ()>;

pub(crate) trait Dyn: 'static + Pointee<Metadata = DynMetadata<Self>> {}
impl<T: ?Sized + 'static + Pointee<Metadata = DynMetadata<T>>> Dyn for T {}

struct ContextLink {
    queue: Box<dyn Tx<Msg>>,
}

pub struct Context {
    internal_messages: LocalQueue<Box<dyn Fn(*mut u8)>>,
    links: HashMap<ContextId, ContextLink>,
    _unsend_marker: PhantomUnsend,
}

type Msg = Box<dyn 'static + Send + FnOnce(*mut u8)>;

impl Context {
    pub(crate) fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    fn send(&mut self, dest: Loc, f: Msg) {
        let conn = self.links.get_mut(&dest.context_id).unwrap();
        conn.queue.send(f);
    }

    pub(crate) fn send_msg<T: ?Sized>(
        &mut self,
        key: Key<T>,
        f: impl 'static + Send + FnOnce(&mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        let queued_fn = Box::new(move |ptr| {
            let ptr = std::ptr::from_raw_parts_mut(ptr as *mut (), key.meta);
            let dest = unsafe { &mut *ptr };
            f(dest)
        });
        self.send(key.loc, queued_fn);
    }
}

#[derive(Clone)]
struct ActorData {
    id: ActorId,
    vtable: &'static ActorVTable,
    loc: Loc,
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
                    loc: Loc {
                        context_id: self.id.unwrap(),
                        offset,
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
            internal_messages: LocalQueue::unbounded(),
            links: HashMap::default(),
            _unsend_marker: Default::default(),
        };

        let tree = self.tree.unwrap();
        for actor in self.actors.unwrap() {
            let init_stage = InitStage {
                context: &mut context,
                tree: &tree,
                actor_being_constructed: actor.id,
            };
            let offset = actor.loc.offset;
            let buf = arena.at_offset(offset.0 as usize, actor.vtable.layout());

            let config = actor_configs.remove(&actor.id).unwrap();
            (actor.vtable.constructor)(&init_stage, buf, config).unwrap();
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

impl InitStage<'_> {
    pub fn query<T: ?Sized>(&self) -> Query<T> {
        Query {
            tree: &self.tree,
            actor_being_constructed: self.actor_being_constructed,
            phantom: PhantomData,
        }
    }
}

pub struct MainStage {
    pub(crate) context: Context,
    pub(crate) actor_running: ActorId,
}

impl MainStage {
    fn broadcast<T>(&mut self, f: impl Send + Clone + Fn(&mut T)) {}
    fn broadcast_mut<T>(
        &mut self,
        group: BroadcastGroup<T>,
        f: impl Send + Clone + FnOnce(&mut T),
    ) {
    }
}
