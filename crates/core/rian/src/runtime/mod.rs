use std::{
    alloc::Layout,
    mem,
    sync::{
        atomic::{self, Ordering},
        Arc,
    },
};

use crate::{
    arena::{Arena, Offset},
    config::ActorConfig,
    context::{
        ActorId, Context, ContextData, ContextId, ContextLink, ControlBlockPtr, InitArgs, InitData,
        MsgRx, MsgTx, QueueItem,
    },
    lookup::{ActorData, ActorTree, Loc},
    object::{ObjectConstructor, VTable},
    queue::{local::LocalQueue, remote},
    Config, Registry,
};

mod graph;

pub fn run(config: Config) {
    let args = create_context_args(config);

    std::thread::scope(|s| {
        let mut args = args.into_iter();
        let fst = args.next().unwrap();
        for a in args {
            s.spawn(|| run_thread(a));
        }
        run_thread(fst);
    });
}

fn create_context_args(config: Config) -> Vec<ContextConstructorArgs> {
    let ns = config.root;
    if !ns.children.is_empty() {
        unimplemented!("Namespaces");
    }
    if !ns.imported_scopes.is_empty() {
        unimplemented!("Namespaces");
    }

    assert!(
        config
            .contexts
            .iter()
            .enumerate()
            .all(|(i, b)| i == b.id.as_index()),
        "Contexts are not named as 1 ..= n"
    );

    struct ContextData {
        tx: remote::Tx<QueueItem>,
        rx: Option<remote::Rx<QueueItem>>,
        actors: Vec<(ActorId, ActorConfig)>,
    }

    let mut contexts: Vec<_> = config
        .contexts
        .iter()
        .map(|_| {
            let (tx, rx) = remote::channel();
            ContextData {
                rx: Some(rx),
                tx,
                actors: Vec::new(),
            }
        })
        .collect();
    let make_tx: Vec<_> = contexts
        .iter()
        .map(|ctx| {
            let tx = ctx.tx.clone();
            Box::new(move || tx.clone()) as _
        })
        .collect();

    let make_tx: Arc<[Box<dyn Send + Sync + 'static + Fn() -> MsgTx>]> = Arc::from(make_tx);
    for (i, c) in ns.actors.into_iter().enumerate() {
        let id = ActorId::new(i as u32 + 1).unwrap();
        contexts[c.context.as_index()].actors.push((id, c));
    }

    let mut constructor_args = Vec::new();
    let mut tree = ActorTree { actors: Vec::new() };
    let control_block_ptr = ControlBlockPtr::new();
    for i in 0..contexts.len() {
        let id = ContextId::new(i as u32 + 1).unwrap();
        let actors = mem::take(&mut contexts[i].actors);
        let (arena, actors) = allocate_actors(actors);
        for actor in &actors {
            tree.actors.push(ActorData {
                id: actor.id,
                vtable: actor.vtable,
                loc: Loc {
                    context_id: id,
                    offset: actor.offset,
                },
            });
        }
        let links = contexts
            .iter()
            .enumerate()
            .filter_map(|(j, ctx)| {
                if i == j {
                    return None;
                }
                Some(ContextLink {
                    queue: ctx.tx.clone(),
                })
            })
            .collect();
        for j in 0..contexts.len() {
            if i == j {
                continue;
            }
        }
        constructor_args.push(ContextConstructorArgs {
            id,
            actors,
            rx: contexts[i].rx.take().unwrap(),
            arena,
            links,
            make_tx: make_tx.clone(),
            tree: None,
            control_block_ptr: control_block_ptr.clone(),
        })
    }
    control_block_ptr.release();
    let tree = Arc::new(tree);

    for ctx in &mut constructor_args {
        ctx.tree = Some(tree.clone());
    }

    constructor_args
}

struct ActorConstructorInfo {
    id: ActorId,
    offset: Offset,
    vtable: &'static VTable,
    cfg: serde_value::Value,
}

struct ContextConstructorArgs {
    id: ContextId,
    actors: Vec<ActorConstructorInfo>,
    rx: MsgRx,
    arena: Arena,
    links: Box<[ContextLink]>,
    tree: Option<Arc<ActorTree>>,
    make_tx: Arc<[Box<dyn Send + Sync + 'static + Fn() -> MsgTx>]>,
    control_block_ptr: ControlBlockPtr,
}

fn allocate_actors(
    configs: impl IntoIterator<Item = (ActorId, ActorConfig)>,
) -> (Arena, Vec<ActorConstructorInfo>) {
    let registry = Registry::get();
    let mut constructor_info: Vec<_> = configs
        .into_iter()
        .map(|(id, cfg)| {
            ActorConstructorInfo {
                id,
                offset: Offset(0), // filled later
                vtable: registry.by_name(&cfg.typename).unwrap().1,
                cfg: cfg.config,
            }
        })
        .collect();
    let (arena, offsets) = Arena::from_layouts(&Vec::from_iter(
        constructor_info.iter().map(|info| info.vtable.layout()),
    ));

    assert_eq!(offsets.len(), constructor_info.len());
    for (info, offset) in constructor_info.iter_mut().zip(offsets) {
        info.offset = offset;
    }

    (arena, constructor_info)
}

fn create_context(info: ContextConstructorArgs) -> (Context, ControlBlockPtr) {
    let ContextConstructorArgs {
        mut arena,
        id,
        actors,
        rx,
        links,
        tree,
        make_tx,
        control_block_ptr,
    } = info;
    let data = ContextData {
        id,
        local_queue: LocalQueue::unbounded(),
        unsent_messages: Vec::default(),
    };

    let mut init_data = InitData {
        data,
        tree: tree.unwrap(),
        dependence_relations: Vec::new(),
        make_tx,
    };

    let drop_fns: Vec<_> = actors.iter().map(|a| (a.offset, a.vtable.drop)).collect();

    for actor in actors {
        let init_stage = InitArgs {
            data: &mut init_data,
            actor_being_constructed: actor.id,
            actor_offset: actor.offset,
            control_block_ptr: &control_block_ptr,
            _phantom: std::marker::PhantomData,
        };
        let cfg = (actor.vtable.deserialize_yaml_value)(actor.cfg).unwrap();
        let offset = actor.offset;
        let buf = arena.at_offset(offset, actor.vtable.layout());
        match actor.vtable.constructor {
            ObjectConstructor::Actor(f) => unsafe { f(init_stage, buf, cfg) }.unwrap(),
        };
    }

    let InitData {
        data,
        dependence_relations,
        tree: _,
        make_tx: _,
    } = init_data;

    if graph::has_cycles(&dependence_relations) {
        panic!("Cycle detected");
    }

    (
        Context {
            data,
            arena,
            drop_fns,
            rx,
            links,
            _unsend_marker: Default::default(),
        },
        control_block_ptr,
    )
}

// Yes, this function is super long and complex
// However, it's better than breaking it up into smaller methods that rely on lots of subtle invariants
fn run_thread(args: ContextConstructorArgs) {
    let (mut ctx, control_block_ptr) = create_context(args);

    // handle first set of local messages. This is separate because we don't want to drop the control block prematurely
    while let Some(msg) = ctx.data.local_queue.recv() {
        msg(&mut ctx);
    }

    // Safety: before a message is pushed to the queue, the control block ptr's ref count is increased.
    // Therefore, accessing control_block_ptr is safe until we decrement it again
    let control_block_ptr = control_block_ptr.into_unowned();

    loop {
        let msg = ctx.rx.recv().unwrap();
        let QueueItem::Msg(msg) = msg else {
            // TODO: how do I indicate this branch is unlikely?
            match msg {
                QueueItem::Msg(_) => unreachable!(),
                QueueItem::AccessorDropped => {
                    let block = unsafe { control_block_ptr.as_ref() };
                    if block.unhandled_events.fetch_sub(1, Ordering::Relaxed) <= 1 {
                        for link in ctx.links.iter() {
                            link.queue.send(QueueItem::Stop).unwrap();
                        }
                        // todo: drop
                        let ptr = control_block_ptr.as_ptr() as _;
                        let layout = Layout::for_value(block);
                        unsafe {
                            std::alloc::dealloc(ptr, layout);
                        }
                        break;
                    } else {
                        continue;
                    }
                }
                QueueItem::Stop => {
                    break;
                }
            }
        };

        msg(&mut ctx);

        // send local messages
        while let Some(msg) = ctx.data.local_queue.recv() {
            msg(&mut ctx);
        }

        // safety: this block is safe to use until we decrement block.unhandled_events
        let block = unsafe { control_block_ptr.as_ref() };
        match ctx.data.unsent_messages.len() {
            0 => {
                if block.unhandled_events.fetch_sub(1, Ordering::Relaxed) <= 1 {
                    // TODO: is this fence necessary?
                    atomic::fence(Ordering::Acquire);
                    for link in ctx.links.iter() {
                        link.queue.send(QueueItem::Stop).unwrap();
                    }
                    break;
                }
                continue;
            }
            1 => {
                let (rx_id, msg) = ctx.data.unsent_messages.pop().unwrap();
                let link = &mut ctx.links[rx_id.as_index() - (ctx.data.id > rx_id) as usize];
                link.queue.send(QueueItem::Msg(msg)).unwrap();
                continue;
            }
            n @ 2.. => {
                block
                    .unhandled_events
                    .fetch_add(n as u32 - 1, Ordering::Relaxed);
            }
        }

        // group messages by thread, then send them out
        // TODO: figure out how to do this faster. We probably can't assume input is nearly sorted.
        // I imagine we may just want to have a preallocated buffer to sort with
        ctx.data.unsent_messages.sort_by_key(|(id, _)| *id);
        let ctx_id = ctx.data.id;
        for (rx_id, msg) in ctx.data.unsent_messages.drain(..) {
            let link = &mut ctx.links[rx_id.as_index() - (ctx_id > rx_id) as usize];
            link.queue.send(QueueItem::Msg(msg)).unwrap();
        }
    }
}
