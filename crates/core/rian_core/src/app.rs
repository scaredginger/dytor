use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{ActorId, ActorTree, Config, Context, ContextId, Registry};

// break it down into stages
// 1. Partition actors into threaded contexts
// 2. Allocate spaces for actors, giving each a Key
// 3. Construct queues between the threads
// 4. For each context, spawn a thread and construct all the objects
// 5. Start handling messages

pub fn run(config: &Config) {
    let ns = &config.root;
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

    let mut context = Context::builder().with_id(ContextId(1));

    let registry = Registry::get();
    let actors_by_name: HashMap<&'static str, TypeId> =
        HashMap::from_iter(registry.actor_types.iter().map(|(k, v)| ((v.name)(), *k)));

    let actors = Vec::from_iter(ns.actors.iter().enumerate().map(|(i, c)| {
        let type_id = actors_by_name.get(c.typename.as_ref()).unwrap();
        let actor = registry.actor_types.get(type_id).unwrap();
        (ActorId(i as u32), actor)
    }));
    let actors = context.place_actors(actors).to_owned();
    context.set_tree(Arc::new(ActorTree { actors }));

    let configs = HashMap::from_iter(ns.actors.iter().enumerate().map(|(i, c)| {
        let type_id = actors_by_name.get(c.typename.as_ref()).unwrap();
        let actor = registry.actor_types.get(type_id).unwrap();
        let config = (actor.deserialize_yaml_value)(&c.config).unwrap();
        (ActorId(i as u32), config)
    }));
    let _context = context.build(configs);
}
