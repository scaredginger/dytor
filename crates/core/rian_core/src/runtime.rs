use std::{
    mem,
    sync::{mpsc, Arc},
};

use crate::{
    actor::ActorVTable,
    arena::{Arena, Offset},
    config::ActorConfig,
    context::{
        ActorId, Context, ContextData, ContextId, ContextLink, InitArgs, InitData, Msg, MsgRx,
        MsgTx,
    },
    lookup::{ActorData, ActorTree, DependenceRelation, Loc},
    queue::local::LocalQueue,
    Config, Registry,
};

pub fn run(config: Config, tokio_rt: &tokio::runtime::Handle) {
    let args = create_context_args(config, tokio_rt);

    std::thread::scope(|s| {
        let mut args = args.into_iter();
        let fst = args.next().unwrap();
        for a in args {
            s.spawn(|| run_thread(a));
        }
        run_thread(fst);
    });
}

fn create_context_args(
    config: Config,
    tokio_rt: &tokio::runtime::Handle,
) -> Vec<ContextConstructorArgs> {
    let ns = config.root;
    if !ns.children.is_empty() {
        unimplemented!("Namespaces");
    }
    if !ns.imported_scopes.is_empty() {
        unimplemented!("Namespaces");
    }

    // let contexts = config
    // .contexts
    // .iter()
    // .map(|ctx| (ctx.id, Context::builder()));
    assert!(
        config
            .contexts
            .iter()
            .enumerate()
            .all(|(i, b)| i == b.id.as_index()),
        "Contexts are not named as 1 ..= n"
    );

    struct ContextData {
        tx: mpsc::Sender<Msg>,
        rx: Option<mpsc::Receiver<Msg>>,
        actors: Vec<(ActorId, ActorConfig)>,
    }

    let mut contexts: Vec<_> = config
        .contexts
        .iter()
        .map(|_| {
            let (tx, rx) = mpsc::channel();
            ContextData {
                rx: Some(rx),
                tx,
                actors: Vec::new(),
            }
        })
        .collect();
    for (i, c) in ns.actors.into_iter().enumerate() {
        let id = ActorId::new(i as u32 + 1).unwrap();
        contexts[c.context.as_index()].actors.push((id, c));
    }

    let mut constructor_args = Vec::new();
    let mut tree = ActorTree { actors: Vec::new() };
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
                    queue: Box::new(ctx.tx.clone()),
                })
            })
            .collect();
        for j in 0..contexts.len() {
            if i == j {
                continue;
            }
        }
        let self_tx = contexts[id.as_index()].tx.clone();
        constructor_args.push(ContextConstructorArgs {
            id,
            actors,
            rx: Box::new(contexts[i].rx.take().unwrap()),
            arena,
            links,
            make_tx: Box::new(move || Box::new(self_tx.clone())),
            tokio_rt: tokio_rt.clone(),
            tree: None,
        })
    }
    let tree = Arc::new(tree);

    for ctx in &mut constructor_args {
        ctx.tree = Some(tree.clone());
    }

    constructor_args
}

struct ActorConstructorInfo {
    id: ActorId,
    offset: Offset,
    vtable: &'static ActorVTable,
    cfg: serde_yaml::Value,
}

struct ContextConstructorArgs {
    id: ContextId,
    actors: Vec<ActorConstructorInfo>,
    rx: MsgRx,
    arena: Arena,
    links: Box<[ContextLink]>,
    tree: Option<Arc<ActorTree>>,
    make_tx: Box<dyn 'static + Send + Fn() -> MsgTx>,
    tokio_rt: tokio::runtime::Handle,
}

pub fn allocate_actors(
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

fn create_context(info: ContextConstructorArgs) -> Context {
    let ContextConstructorArgs {
        mut arena,
        id,
        actors,
        rx,
        links,
        tree,
        make_tx,
        tokio_rt,
    } = info;
    let data = ContextData {
        id,
        local_queue: LocalQueue::unbounded(),
        links,
        rx,
    };

    let mut init_data = InitData {
        data,
        tree: tree.unwrap(),
        dependence_relations: Vec::new(),
        make_tx,
        tokio_rt,
    };

    for actor in actors {
        let init_stage = InitArgs {
            data: &mut init_data,
            actor_being_constructed: actor.id,
            actor_offset: actor.offset,
            _phantom: std::marker::PhantomData,
        };
        let cfg = (actor.vtable.deserialize_yaml_value)(actor.cfg).unwrap();
        let offset = actor.offset;
        let buf = arena.at_offset(offset, actor.vtable.layout());
        unsafe { (actor.vtable.constructor)(init_stage, buf, cfg) }.unwrap();
    }

    let InitData {
        data,
        dependence_relations,
        tree: _,
        make_tx: _,
        tokio_rt: _,
    } = init_data;

    if has_cycles(dependence_relations) {
        panic!("Cycle detected");
    }

    Context {
        data,
        arena,
        _unsend_marker: Default::default(),
    }
}

fn has_cycles(relations: Vec<DependenceRelation>) -> bool {
    unimplemented!()
}

fn run_thread(args: ContextConstructorArgs) {
    let mut ctx = create_context(args);
    while let Ok(msg) = ctx.data.rx.recv() {
        msg(&mut ctx);
        while let Ok(msg) = ctx.data.local_queue.recv() {
            msg(&mut ctx);
        }
    }
}
