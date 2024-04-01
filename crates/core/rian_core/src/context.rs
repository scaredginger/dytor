use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::Pointee,
};

use serde::Deserialize;

use crate::{
    arena::{Arena, Offset},
    lookup::{ActorTree, BroadcastGroup, Key, Query},
    queue::{Rx, Tx},
};

type PhantomUnsend = PhantomData<*mut ()>;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ActorId(pub(crate) u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct ContextId(pub(crate) u32);

impl ContextId {
    #[must_use]
    pub fn from_u32(x: u32) -> Self {
        Self(x)
    }
}

type LocalQueue = crate::queue::local::LocalQueue<Box<dyn Fn(&mut Context)>>;

pub struct Context {
    pub(crate) context_id: ContextId,
    pub(crate) local_queue: LocalQueue,
    pub(crate) links: HashMap<ContextId, ContextLink>,
    pub(crate) rx: MsgRx,
    pub(crate) arena: Arena,
    pub(crate) _unsend_marker: PhantomUnsend,
}

pub(crate) type Msg = Box<dyn 'static + Send + FnOnce(&mut Context)>;
pub(crate) type MsgRx = Box<dyn Rx<Msg>>;
pub(crate) type MsgTx = Box<dyn Tx<Msg>>;

impl Context {
    pub(crate) fn process_msg(&mut self, msg: Msg) {
        msg(self);
    }
}

pub(crate) struct ContextLink {
    queue: MsgTx,
}

impl ContextLink {
    pub(crate) fn send_msg<T: ?Sized>(
        &mut self,
        key: Key<T>,
        f: impl 'static + Send + FnOnce(&mut T),
    ) -> Result<(), ()>
    where
        <T as Pointee>::Metadata: 'static,
    {
        let offset = key.loc.offset;
        let meta = key.meta;
        let queued_fn = Box::new(move |ctx: &mut Context| {
            let ptr = ctx.arena.offset(offset);
            let ptr = std::ptr::from_raw_parts_mut(ptr as *mut (), meta);
            f(unsafe { &mut *ptr })
        });
        self.queue.send(queued_fn).map_err(|_| ())
    }
}

pub struct SendStage<'a> {
    pub(crate) links: &'a mut HashMap<ContextId, ContextLink>,
    pub(crate) context_id: ContextId,
    pub(crate) local_queue: &'a mut LocalQueue,
}

pub struct InitStage<'init> {
    pub(crate) send_stage: SendStage<'init>,
    pub(crate) tree: &'init ActorTree,
    pub(crate) actor_being_constructed: ActorId,
}

impl<'a> Deref for InitStage<'a> {
    type Target = SendStage<'a>;

    fn deref(&self) -> &Self::Target {
        &self.send_stage
    }
}

impl<'a> DerefMut for InitStage<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.send_stage
    }
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

pub struct MainStage<'a> {
    send_stage: SendStage<'a>,
}

impl<'a> Deref for MainStage<'a> {
    type Target = SendStage<'a>;

    fn deref(&self) -> &Self::Target {
        &self.send_stage
    }
}

impl<'a> DerefMut for MainStage<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.send_stage
    }
}

impl<'a> SendStage<'a> {
    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainStage, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        for (id, refs) in group.by_context.as_ref() {
            if *id == self.context_id {
                let refs = refs.clone();
                let f = f.clone();
                self.local_queue.send(Box::new(move |ctx: &mut Context| {
                    for (offset, meta) in refs.as_ref() {
                        let ptr = std::ptr::from_raw_parts_mut(
                            ctx.arena.offset(*offset) as *mut (),
                            *meta,
                        );
                        let mut ms = MainStage {
                            send_stage: SendStage {
                                links: &mut ctx.links,
                                context_id: ctx.context_id,
                                local_queue: &mut ctx.local_queue,
                            },
                        };
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                }));
            } else {
                let link = self.links.get_mut(id).unwrap();
                let refs = refs.clone();
                let f = f.clone();
                link.queue.send(Box::new(move |ctx| {
                    for (offset, meta) in refs.as_ref() {
                        let ptr = ctx.arena.offset(*offset);
                        let ptr = std::ptr::from_raw_parts_mut(ptr as *mut (), *meta);
                        let mut ms = MainStage {
                            send_stage: SendStage {
                                links: &mut ctx.links,
                                context_id: ctx.context_id,
                                local_queue: &mut ctx.local_queue,
                            },
                        };
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                }));
            }
        }
    }
}
