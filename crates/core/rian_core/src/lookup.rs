use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    ptr::{DynMetadata, Pointee},
    sync::Arc,
};

use crate::{
    actor::{ActorVTable, TraitId},
    arena::Offset,
    context::ActorId,
    ContextId, Registry,
};

#[derive(Clone)]
pub(crate) struct ActorData {
    pub(crate) id: ActorId,
    pub(crate) vtable: &'static ActorVTable,
    pub(crate) loc: Loc,
}

#[derive(Default)]
pub(crate) struct ActorTree {
    // TODO: actually make this a tree
    pub(crate) actors: Vec<ActorData>,
}

pub(crate) trait Lookup<T: ?Sized, D> {
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = (ActorId, Key<T>)>;
}

impl<T: 'static> Lookup<T, ()> for ActorTree
where
    T: Pointee<Metadata = ()>,
{
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = (ActorId, Key<T>)> {
        let type_id = TypeId::of::<T>();
        self.actors
            .iter()
            .filter(move |actor| actor.vtable.type_id == type_id)
            .map(|actor| {
                (
                    actor.id,
                    Key {
                        loc: actor.loc,
                        meta: (),
                    },
                )
            })
    }
}

impl<T: ?Sized + 'static> Lookup<T, DynMetadata<T>> for ActorTree
where
    T: Pointee<Metadata = DynMetadata<T>>,
{
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = (ActorId, Key<T>)> {
        let trait_id = TraitId::of::<T>();
        let types: &[_] = Registry::get()
            .trait_types
            .get(&trait_id)
            .map_or(&[], AsRef::as_ref);
        self.actors
            .iter()
            .filter_map(|actor| {
                Some((
                    actor,
                    types.iter().find(|x| x.type_id == actor.vtable.type_id)?,
                ))
            })
            .map(|(actor, t)| {
                (
                    actor.id,
                    Key {
                        loc: actor.loc,
                        meta: unsafe { std::mem::transmute(t.dyn_meta) },
                    },
                )
            })
    }
}

pub(crate) struct DependenceRelation {
    from: ActorId,
    to: ActorId,
}

pub struct Query<'a, T: ?Sized> {
    pub(crate) dependence_relations: &'a mut Vec<DependenceRelation>,
    pub(crate) tree: &'a ActorTree,
    pub(crate) actor_being_constructed: ActorId,
    pub(crate) curr_context: ContextId,
    pub(crate) phantom: PhantomData<T>,
}

impl<'a, T: 'static + ?Sized> Query<'a, T>
where
    ActorTree: Lookup<T, <T as Pointee>::Metadata>,
{
    pub fn all_keys(self) -> impl 'a + Iterator<Item = (ActorId, Key<T>)> {
        self.tree.lookup(self.actor_being_constructed)
    }

    pub fn broadcast_group(self) -> BroadcastGroup<T> {
        let mut map: HashMap<_, Vec<_>> = HashMap::new();
        for (_, key) in self.tree.lookup(self.actor_being_constructed) {
            map.entry(key.loc.context_id)
                .or_default()
                .push((key.loc.offset, key.meta));
        }

        let by_context = map
            .into_iter()
            .map(|(context_id, vec)| (context_id, vec.into()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        BroadcastGroup { by_context }
    }

    pub fn acyclic_local_key(self) -> AcyclicLocalKey<T> {
        let mut it = self.tree.lookup(self.actor_being_constructed);
        let (local_actor_id, local_actor_key) = it.next().unwrap();
        assert!(it.next().is_none());
        assert!(local_actor_key.loc.context_id == self.curr_context);

        self.dependence_relations.push(DependenceRelation {
            from: self.actor_being_constructed,
            to: local_actor_id,
        });

        AcyclicLocalKey {
            offset: local_actor_key.loc.offset,
            meta: local_actor_key.meta,
        }
    }
}

impl<'a, T: ?Sized + 'static> From<Query<'a, T>> for BroadcastGroup<T>
where
    ActorTree: Lookup<T, <T as Pointee>::Metadata>,
{
    fn from(value: Query<T>) -> Self {
        value.broadcast_group()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Loc {
    pub(crate) context_id: ContextId,
    pub(crate) offset: Offset,
}

pub struct BroadcastGroup<T: ?Sized> {
    pub(crate) by_context: Box<[(ContextId, Arc<[(Offset, <T as Pointee>::Metadata)]>)]>,
}

#[derive(Clone, Copy)]
pub struct Key<T: ?Sized> {
    pub(crate) loc: Loc,
    pub(crate) meta: <T as Pointee>::Metadata,
}

#[derive(Clone, Copy)]
pub struct AcyclicLocalKey<T: ?Sized> {
    pub(crate) offset: Offset,
    pub(crate) meta: <T as Pointee>::Metadata,
}

impl<'a, T: ?Sized + 'static> From<Query<'a, T>> for AcyclicLocalKey<T>
where
    ActorTree: Lookup<T, <T as Pointee>::Metadata>,
{
    fn from(value: Query<T>) -> Self {
        value.acyclic_local_key()
    }
}
