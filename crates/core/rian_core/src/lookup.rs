use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    ptr::{DynMetadata, Pointee},
    sync::Arc,
};

use crate::{actor::TraitId, arena::Offset, ActorData, ActorId, ContextId, MainStage, Registry};

#[derive(Default)]
pub(crate) struct ActorTree {
    // TODO: actually make this a tree
    pub(crate) actors: Vec<ActorData>,
}

pub(crate) trait Lookup<T: ?Sized, D> {
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = Key<T>>;
}

impl<T: 'static> Lookup<T, ()> for ActorTree
where
    T: Pointee<Metadata = ()>,
{
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = Key<T>> {
        let type_id = TypeId::of::<T>();
        self.actors
            .iter()
            .filter(move |actor| actor.vtable.type_id == type_id)
            .map(|actor| Key {
                loc: actor.loc,
                meta: (),
            })
    }
}

impl<T: ?Sized + 'static> Lookup<T, DynMetadata<T>> for ActorTree
where
    T: Pointee<Metadata = DynMetadata<T>>,
{
    fn lookup(&self, from_actor: ActorId) -> impl '_ + Iterator<Item = Key<T>> {
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
            .map(|(actor, t)| Key {
                loc: actor.loc,
                meta: unsafe { std::mem::transmute(t.dyn_meta) },
            })
    }
}

pub struct Query<'a, T: ?Sized> {
    pub(crate) tree: &'a ActorTree,
    pub(crate) actor_being_constructed: ActorId,
    pub(crate) phantom: PhantomData<T>,
}

impl<'a, T: 'static + ?Sized> Query<'a, T>
where
    ActorTree: Lookup<T, <T as Pointee>::Metadata>,
{
    pub fn all_refs(self) -> impl 'a + Iterator<Item = Key<T>> {
        self.tree.lookup(self.actor_being_constructed)
    }

    pub fn broadcast_group(self) -> BroadcastGroup<T> {
        let mut map: HashMap<_, Vec<_>> = HashMap::new();
        for key in self.tree.lookup(self.actor_being_constructed) {
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

impl<T: ?Sized> Key<T>
where
    <T as Pointee>::Metadata: 'static,
{
    fn send(self, stage: &mut MainStage, f: impl 'static + Send + FnOnce(&mut T)) {
        stage.context.send_msg(self, f);
    }
}
