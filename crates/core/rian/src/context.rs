use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    num::NonZeroU32,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull, Pointee},
    sync::{
        atomic::{fence, AtomicU32, Ordering},
        Arc,
    },
};

use serde::Deserialize;

use crate::{
    arena::{Arena, Offset},
    lookup::{ActorTree, BroadcastGroup, DependenceRelation, Key, Query},
    queue::remote,
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
    pub(crate) unsent_messages: Vec<(ContextId, Msg)>,
}

type DropFn = unsafe fn(*mut u8);

// TODO: move this to runtime module
pub struct Context {
    pub(crate) data: ContextData,
    pub(crate) arena: Arena,
    pub(crate) drop_fns: Vec<(Offset, DropFn)>,
    pub(crate) rx: MsgRx,
    pub(crate) links: Box<[ContextLink]>,
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

pub enum QueueItem {
    Msg(Msg),
    AccessorDropped,
    Stop,
}

pub(crate) type Msg = Box<dyn 'static + Send + FnOnce(&mut Context)>;
pub(crate) type MsgRx = remote::Rx<QueueItem>;
pub(crate) type MsgTx = remote::Tx<QueueItem>;

pub(crate) struct ContextLink {
    pub(crate) queue: MsgTx,
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
    pub(crate) control_block_ptr: &'a ControlBlockPtr,
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
            f(&mut args, unsafe { &mut *ptr });
        });
        if self.data.id == loc.context_id {
            self.data.local_queue.send(f)
        } else {
            self.data.unsent_messages.push((loc.context_id, f));
        }
    }

    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: &BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        self.data.broadcast(group, f)
    }
}

impl<'a, ActorT: 'static> InitArgs<'a, ActorT> {
    pub fn accessor(&self) -> Accessor<ActorT> {
        mem::forget(self.control_block_ptr.clone());
        Accessor {
            offset: self.actor_offset,
            metadata: (),
            ctx_queue: (self.data.make_tx[self.data.id.as_index()])(),
            control_block_ptr: self.control_block_ptr.0,
            _phantom: PhantomData,
        }
    }

    pub fn accessor_for_key<T: 'static + ?Sized>(&self, key: Key<T>) -> Accessor<T> {
        mem::forget(self.control_block_ptr.clone());
        Accessor {
            offset: key.loc.offset,
            metadata: key.meta,
            ctx_queue: (self.data.make_tx[key.loc.context_id.as_index()])(),
            control_block_ptr: self.control_block_ptr.0,
            _phantom: PhantomData,
        }
    }
}

pub struct MainArgs<'a> {
    pub(crate) context_data: &'a mut ContextData,
    pub(crate) arena: &'a Arena,
}

impl<'a> MainArgs<'a> {
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
            f(&mut args, unsafe { &mut *ptr });
        });
        if self.context_data.id == loc.context_id {
            self.context_data.local_queue.send(f)
        } else {
            self.context_data.unsent_messages.push((loc.context_id, f));
        }
    }

    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: &BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        self.context_data.broadcast(group, f)
    }
}

impl ContextData {
    pub fn broadcast<T: ?Sized>(
        &mut self,
        group: &BroadcastGroup<T>,
        f: impl 'static + Send + Clone + Fn(&mut MainArgs, &mut T),
    ) where
        <T as Pointee>::Metadata: 'static,
    {
        for (id, refs) in group.by_context.as_ref() {
            if *id == self.id {
                let refs = refs.clone();
                let f = f.clone();
                self.local_queue.send(Box::new(move |ctx: &mut Context| {
                    let mut ms = MainArgs {
                        context_data: &mut ctx.data,
                        arena: &ctx.arena,
                    };
                    for (offset, meta) in refs.as_ref() {
                        let ptr =
                            ptr::from_raw_parts_mut(ctx.arena.offset(*offset) as *mut (), *meta);
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                }));
            } else {
                let refs = refs.clone();
                let f = f.clone();
                let msg = Box::new(move |ctx: &mut Context| {
                    let mut ms = MainArgs {
                        context_data: &mut ctx.data,
                        arena: &ctx.arena,
                    };
                    for (offset, meta) in refs.as_ref() {
                        let ptr = ctx.arena.offset(*offset);
                        let ptr = ptr::from_raw_parts_mut(ptr as *mut (), *meta);
                        f(&mut ms, unsafe { &mut *ptr });
                    }
                });
                self.unsent_messages.push((*id, msg));
            }
        }
    }
}

pub(crate) struct ControlBlock {
    pub(crate) unhandled_events: AtomicU32,
}

pub(crate) struct ControlBlockPtr(pub(crate) NonNull<ControlBlock>);

unsafe impl Send for ControlBlockPtr {}

impl ControlBlockPtr {
    pub(crate) fn new() -> Self {
        let block = ControlBlock {
            unhandled_events: AtomicU32::new(1),
        };
        let layout = Layout::for_value(&block);
        let ptr = unsafe { std::alloc::alloc(layout) } as *mut MaybeUninit<ControlBlock>;
        let cb = unsafe { ptr.as_mut() }.unwrap();
        cb.write(block);
        let ptr = NonNull::new(ptr as _).unwrap();
        Self(ptr)
    }

    pub(crate) fn release(self) -> bool {
        let ptr = self.0.as_ptr();
        let block = unsafe { &*ptr };
        std::mem::forget(self);
        if block.unhandled_events.fetch_sub(1, Ordering::Release) > 1 {
            return false;
        }

        fence(Ordering::Acquire);
        let layout = Layout::for_value(&block);
        unsafe { std::alloc::dealloc(ptr as *mut _, layout) };
        true
    }

    pub(crate) fn into_unowned(self) -> NonNull<ControlBlock> {
        let res = self.0;
        self.release();
        res
    }
}

impl Clone for ControlBlockPtr {
    fn clone(&self) -> Self {
        let block = unsafe { self.0.as_ref() };
        block.unhandled_events.fetch_add(1, Ordering::Relaxed);
        Self(self.0)
    }
}

impl Drop for ControlBlockPtr {
    #[inline(always)]
    fn drop(&mut self) {
        unreachable!(
            "A control block ptr should never be dropped. Destroy it with one of the methods."
        );
    }
}

pub struct Accessor<T: ?Sized> {
    pub(crate) offset: Offset,
    pub(crate) metadata: <T as Pointee>::Metadata,
    pub(crate) ctx_queue: remote::Tx<QueueItem>,
    pub(crate) control_block_ptr: NonNull<ControlBlock>,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T: ?Sized> Drop for Accessor<T> {
    fn drop(&mut self) {
        self.ctx_queue.send(QueueItem::AccessorDropped).unwrap();
    }
}

/// safety: only _phantom stops it from being send
unsafe impl<T: ?Sized> Send for Accessor<T> {}

impl<T: ?Sized + 'static> Accessor<T> {
    pub fn send(&self, f: impl 'static + Send + FnOnce(&mut MainArgs, &mut T)) {
        let offset = self.offset;
        let metadata = self.metadata;
        let queued_fn = Box::new(move |ctx: &mut Context| {
            let ptr = ctx.arena.offset(offset);
            let ptr = ptr::from_raw_parts_mut(ptr as *mut (), metadata);
            let mut ms = MainArgs {
                context_data: &mut ctx.data,
                arena: &ctx.arena,
            };
            f(&mut ms, unsafe { &mut *ptr });
        });
        unsafe { self.control_block_ptr.as_ref() }
            .unhandled_events
            .fetch_add(1, Ordering::Relaxed);
        self.ctx_queue.send(QueueItem::Msg(queued_fn)).unwrap()
    }
}
