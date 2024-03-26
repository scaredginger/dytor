use std::ops::Range;

use futures::{stream::FuturesUnordered, StreamExt as _};
use rian::{Context, InitData, MainData, Registry};

use crate::{arena::compute_space_bound, Config};

pub async fn main(config: &Config) {
    let ns = &config.root_namespace;
    if !ns.children.is_empty() {
        unimplemented!("Namespaces");
    }
    if !ns.namespace_imports.is_empty() {
        unimplemented!("Namespaces");
    }

    let _libs: Vec<_> = config
        .shared_lib_paths
        .iter()
        .map(|p| {
            let s = p.as_os_str();
            unsafe { libloading::Library::new(s) }.unwrap()
        })
        .collect();

    let registry = Registry::load();
    let context = Context::new(registry.clone());

    let actors = ns.actors.iter().map(|c| {
        let actor = registry.actors.get(c.typename.as_ref()).unwrap();
        actor.layout()
    });
    let space_to_alloc = compute_space_bound(actors);
    let mut data: Box<[u8]> = vec![0; space_to_alloc].into_boxed_slice();

    let Range { mut start, end } = data.as_mut_ptr_range();

    let init_data = InitData::from(context);

    let mut refs = Vec::new();
    for c in &ns.actors {
        let actor = registry.actors.get(c.typename.as_ref()).unwrap();
        let addr = start.wrapping_add(start.align_offset(actor.layout().align()));
        start = addr.wrapping_add(actor.layout().size());
        assert!(start <= end);
        let dest = unsafe { std::slice::from_raw_parts_mut(addr, actor.layout().size()) };
        refs.push((addr, *actor));
        let actor_config = (actor.deserialize_yaml_value)(&c.config).unwrap();
        (actor.constructor)(&init_data, dest, actor_config).unwrap();
    }

    let main_data = MainData::from(init_data);

    let mut fut = FuturesUnordered::from_iter(
        refs.iter()
            .map(|&(addr, info)| (info.run)(addr, &main_data)),
    );
    while let Some(next) = fut.next().await {
        next.unwrap();
    }
    let mut fut =
        FuturesUnordered::from_iter(refs.iter().map(|&(addr, info)| (info.terminate)(addr)));
    while let Some(_) = fut.next().await {}

    for (addr, info) in refs {
        (info.drop)(addr);
    }
}
