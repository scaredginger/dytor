use std::{
    marker::PhantomData,
    num::NonZeroU32,
    ops::{Deref, DerefMut},
    ptr::{self, Pointee},
    sync::Arc,
};

use serde::Deserialize;

use crate::{
    arena::{Arena, Offset},
    lookup::{ActorTree, BroadcastGroup, DependenceRelation, Key, Query},
    queue::{Rx, Tx, WriteErr, WriteResult},
};

type PhantomUnsend = PhantomData<*mut ()>;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ActorId(pub(crate) NonZeroU32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct ContextId(pub(crate) NonZeroU32);

macro_rules! impl_inner_ops {
    ($struct_name:ident) => {
        #[allow(unused)]
        impl $struct_name {
            #[must_use]
            pub fn new(x: u32) -> Option<Self> {
                Some(Self(std::num::NonZeroU32::new(x)?))
            }

            #[must_use]
            pub(crate) fn as_u32(self) -> u32 {
                self.0.into()
            }

            #[must_use]
            pub(crate) fn as_index(self) -> usize {
                (self.as_u32() - 1) as usize
            }
        }
    };
}

impl_inner_ops!(ActorId);
impl_inner_ops!(ContextId);

type LocalQueue = crate::queue::local::LocalQueue<Box<dyn FnOnce(&mut Context)>>;

pub(crate) struct ContextData {
    pub(crate) id: ContextId,
    pub(crate) local_queue: LocalQueue,
    pub(crate) links: Box<[ContextLink]>,
    pub(crate) rx: MsgRx,
}

pub struct Context {
    pub(crate) data: ContextData,
    pub(crate) arena: Arena,
    pub(crate) drop_fns: Vec<(Offset, unsafe fn(*mut u8))>,
    pub(crate) _unsend_marker: PhantomUnsend,
}

impl Drop for Context {
    fn drop(&mut self) {
        for &(offset, drop) in &self.drop_fns {
            let ptr = self.arena.offset(offset);
            unsafe { drop(ptr) };
        }
    }
}

pub(crate) type Msg = Box<dyn 'static + Send + FnOnce(&mut Context)>;
pub(crate) type MsgRx = Box<dyn Rx<Msg>>;
pub(crate) type MsgTx = Box<dyn Tx<Msg>>;

impl ContextData {
    fn link_mut(&mut self, other: ContextId) -> &mut ContextLink {
        &mut self.links[other.as_index() - (self.id > other) as usize]
    }
}

pub(crate) struct ContextLink {
    pub(crate) queue: MsgTx,
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
            let ptr = ptr::from_raw_parts_mut(ptr as *mut (), meta);
            f(unsafe { &mut *ptr })
        });
        self.queue.send(queued_fn).map_err(|_| ())
    }
}

pub(crate) struct InitData {
    pub(crate) data: ContextData,
    pub(crate) tree: Arc<ActorTree>,
    pub(crate) dependence_relations: Vec<DependenceRelation>,
    pub(crate) make_tx: Arc<[Box<dyn Fn() -> MsgTx + Send + Sync>]>,
}

pub struct InitArgs<'a, ActorT> {
    pub(crate) data: &'a mut InitData,
    pub(crate) actor_being_constructed: ActorId,
    pub(crate) actor_offset: Offset,
    pub(crate) _phantom: PhantomData<ActorT>,
}

impl Deref for InitData {
    type Target = ContextData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for InitData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, ActorT> InitArgs<'a, ActorT> {
    pub fn query<T: ?Sized>(&mut self) -> Query<'_, 'a, T, ActorT> {
        Query {
            init_args: self,
            phantom: PhantomData,
        }
    }

    pub fn send_msg<T: ?Sized>(
        &mut self,
        Key { loc, meta }: Key<T>,
        f: impl 'static + Send + FnOnce(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        let f = Box::new(move |ctx: &mut Context| {
            let ptr = ctx.arena.offset(loc.offset);
            let ptr = ptr::from_raw_parts_mut(ptr as *mut (), meta);
            let mut args = MainArgs {
                context_data: &mut ctx.data,
                arena: &ctx.arena,
            };
            f(&mut args, unsafe { &mut *ptr })
        });
        if self.data.id == loc.context_id {
            self.data
                .local_queue
                .send(f)
                .unwrap_or_else(|_| panic!("Local queue full"));
        } else {
            self.data
                .link_mut(loc.context_id)
                .queue
                .send(f)
                .unwrap_or_else(|_| panic!("Local queue full"));
        }
    }

    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        self.data.broadcast(group, f)
    }
}

impl<'a, ActorT: 'static> InitArgs<'a, ActorT> {
    pub fn accessor(&self) -> Accessor<ActorT> {
        Accessor {
            offset: self.actor_offset,
            metadata: (),
            ctx_queue: (&self.data.make_tx[self.data.id.as_index()])(),
            _phantom: PhantomData,
        }
    }

    pub fn accessor_for_key<T: 'static + ?Sized>(&self, key: Key<T>) -> Accessor<T> {
        Accessor {
            offset: key.loc.offset,
            metadata: key.meta,
            ctx_queue: (&self.data.make_tx[key.loc.context_id.as_index()])(),
            _phantom: PhantomData,
        }
    }
}

pub struct MainArgs<'a> {
    pub(crate) context_data: &'a mut ContextData,
    pub(crate) arena: &'a Arena,
}

impl<'a> MainArgs<'a> {}

impl ContextData {
    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        for (id, refs) in group.by_context.as_ref() {
            if *id == self.id {
                let refs = refs.clone();
                let f = f.clone();
                self.local_queue.send(Box::new(move |ctx: &mut Context| {
                    for (offset, meta) in refs.as_ref() {
                        let ptr =
                            ptr::from_raw_parts_mut(ctx.arena.offset(*offset) as *mut (), *meta);
                        let mut ms = MainArgs {
                            context_data: &mut ctx.data,
                            arena: &ctx.arena,
                        };
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                }));
            } else {
                let refs = refs.clone();
                let f = f.clone();
                self.link_mut(*id).queue.send(Box::new(move |ctx| {
                    for (offset, meta) in refs.as_ref() {
                        let ptr = ctx.arena.offset(*offset);
                        let ptr = ptr::from_raw_parts_mut(ptr as *mut (), *meta);
                        let mut ms = MainArgs {
                            context_data: &mut ctx.data,
                            arena: &ctx.arena,
                        };
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                }));
            }
        }
    }
}

pub struct Accessor<T: ?Sized> {
    pub(crate) offset: Offset,
    pub(crate) metadata: <T as Pointee>::Metadata,
    pub(crate) ctx_queue: Box<dyn 'static + Tx<Msg>>,
    pub(crate) _phantom: PhantomData<T>,
}

/// safety: only _phantom stops it from being send
unsafe impl<T: ?Sized> Send for Accessor<T> {}

impl<T: ?Sized + 'static> Accessor<T> {
    pub fn send(
        &mut self,
        f: impl 'static + Send + FnOnce(MainArgs, &mut T) -> (),
    ) -> WriteResult<()> {
        let offset = self.offset;
        let metadata = self.metadata;
        let queued_fn = Box::new(move |ctx: &mut Context| {
            let ptr = ctx.arena.offset(offset);
            let ptr = ptr::from_raw_parts_mut(ptr as *mut (), metadata);
            let ms = MainArgs {
                context_data: &mut ctx.data,
                arena: &ctx.arena,
            };
            f(ms, unsafe { &mut *ptr });
        });
        self.ctx_queue.send(queued_fn).map_err(move |e| match e {
            WriteErr::Finished(_) => WriteErr::Finished(()),
            WriteErr::Full(_) => WriteErr::Full(()),
        })
    }
}
