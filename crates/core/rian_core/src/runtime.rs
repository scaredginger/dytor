use std::{any::Any, collections::HashMap, sync::Arc};

use crate::{
    arena::Arena,
    config::ActorConfig,
    context::{ActorId, Context, ContextId, InitStage, MsgRx, SendStage},
    lookup::{ActorData, ActorTree, Loc},
    queue::{local::LocalQueue, Rx},
    MainStage, Registry,
};

pub(crate) struct ContextInfo {
    id: ContextId,
    pub(crate) actors: Vec<ActorData>,
    actor_configs: Vec<Box<dyn Any + Send>>,
    rx: MsgRx,
    arena: Arena,
}

impl ContextInfo {
    pub(crate) fn new(
        context_id: ContextId,
        actor_configs: Vec<(ActorId, &ActorConfig)>,
        rx: MsgRx,
    ) -> Self {
        let registry = Registry::get();
        let vtables: Vec<_> = actor_configs
            .iter()
            .map(|(id, cfg)| (id, registry.by_name(&cfg.typename).unwrap().1))
            .collect();
        let actor_configs = actor_configs
            .iter()
            .zip(vtables.iter())
            .map(|((_, cfg), (_, vtable))| (vtable.deserialize_yaml_value)(&cfg.config).unwrap())
            .collect();
        let (arena, offsets) = Arena::from_layouts(&Vec::from_iter(
            vtables.iter().map(|(_, vtable)| vtable.layout()),
        ));
        let actors = offsets
            .into_iter()
            .zip(vtables)
            .map(|(offset, (&id, vtable))| ActorData {
                id,
                vtable,
                loc: Loc { context_id, offset },
            })
            .collect();
        Self {
            id: context_id,
            actors,
            arena,
            actor_configs,
            rx,
        }
    }
}

fn run_init_stage(info: ContextInfo, tree: &ActorTree) -> Context {
    let ContextInfo {
        arena,
        actor_configs,
        id,
        actors,
        rx,
    } = info;
    let mut context = Context {
        context_id: id,
        arena,
        local_queue: LocalQueue::unbounded(),
        links: HashMap::default(),
        rx,
        _unsend_marker: Default::default(),
    };

    for (actor, config) in actors.into_iter().zip(actor_configs) {
        let mut init_stage = InitStage {
            send_stage: SendStage {
                links: &mut context.links,
                context_id: context.context_id,
                local_queue: &mut context.local_queue,
            },
            tree: &tree,
            actor_being_constructed: actor.id,
        };
        let offset = actor.loc.offset;
        let buf = context.arena.at_offset(offset, actor.vtable.layout());
        (actor.vtable.constructor)(&mut init_stage, buf, config).unwrap();
    }

    context
}

fn run_main_stage(ctx: &mut Context) {
    while let Ok(msg) = ctx.local_queue.recv() {
        msg(ctx);
    }
}

fn run_thread(info: ContextInfo, tree: Arc<ActorTree>) {
    let mut ctx = run_init_stage(info, &tree);
    drop(tree);
    run_main_stage(&mut ctx);
}

pub(crate) fn run(contexts: impl IntoIterator<Item = ContextInfo>, tree: Arc<ActorTree>) {
    std::thread::scope(move |scope| {
        let mut it = contexts.into_iter();
        let first = it.next();
        for info in it {
            let tree = tree.clone();
            scope.spawn(move || run_thread(info, tree));
        }
        first.map(move |info| run_thread(info, tree));
    })
}
