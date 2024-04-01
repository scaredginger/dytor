use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{
    context::ActorId, context::Context, lookup::ActorTree, runtime::ContextInfo, Config, ContextId,
    Registry,
};

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

    let actors = Vec::from_iter(
        ns.actors
            .iter()
            .enumerate()
            .map(|(i, c)| (ActorId(i as u32), c)),
    );
    let (_, rx) = std::sync::mpsc::channel();
    let info = ContextInfo::new(ContextId(1), actors, Box::new(rx));

    let mut at = ActorTree::default();
    at.actors.extend(info.actors.iter().cloned());

    crate::runtime::run([info], Arc::new(at));
    // let _context = context.build(configs);
}
