use std::{collections::HashMap, ops::Range, path::Path, sync::Arc};

use futures::stream::{FuturesUnordered, StreamExt as _};
use libloading::Symbol;
use serde::Deserialize;

use crate::{InitData, MainData, Registry};

#[derive(Deserialize)]
pub struct ActorConfig {
    pub typename: Arc<str>,
    pub config: serde_yaml::Value,
}

pub struct NamespacePath(Vec<Arc<str>>);

impl<'de> Deserialize<'de> for NamespacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        Ok(Self(s.split("::").map(|x| Arc::from(x)).collect()))
    }
}

#[derive(Deserialize)]
pub struct Namespace {
    pub children: HashMap<Arc<str>, Namespace>,
    pub actors: Vec<ActorConfig>,
    pub namespace_imports: Vec<NamespacePath>,
}

#[derive(Deserialize)]
pub struct Config {
    pub root_namespace: Namespace,
    pub shared_lib_paths: Vec<Arc<Path>>,
}

fn compute_space_bound<'a>(size_align: impl IntoIterator<Item = (usize, usize)>) -> usize {
    let mut res: usize = 0;
    let mut known_align: usize = 1;
    let mut curr_offset: usize = 0;

    for (size, align) in size_align {
        if align > known_align {
            res += align - known_align + (known_align - curr_offset) % known_align;
            known_align = align;
            curr_offset = 0;
        } else {
            let padding = align - ((curr_offset + align - 1) % align) - 1;
            res += padding;
            curr_offset = (curr_offset + padding) % known_align;
        }
        res += size;
        curr_offset = (curr_offset + size) % known_align;
    }
    res
}

pub async fn main(config: &Config) {
    let ns = &config.root_namespace;
    if !ns.children.is_empty() {
        unimplemented!("Namespaces");
    }
    if !ns.namespace_imports.is_empty() {
        unimplemented!("Namespaces");
    }

    let mut registry = Registry::default();
    let libs: Vec<_> = config
        .shared_lib_paths
        .iter()
        .map(|p| {
            let s = p.as_os_str();
            unsafe { libloading::Library::new(s) }.unwrap()
        })
        .collect();
    for lib in &libs {
        let init_fn: Symbol<unsafe extern "C" fn(&mut Registry)> =
            unsafe { lib.get(b"load_rian_module\0") }.unwrap();

        unsafe {
            init_fn(&mut registry);
        }
    }

    let actors = ns.actors.iter().map(|c| {
        let actor = registry.actors.get(c.typename.as_ref()).unwrap();
        (actor.size, actor.align)
    });
    let space_to_alloc = compute_space_bound(actors);
    let mut data: Box<[u8]> = vec![0; space_to_alloc].into_boxed_slice();

    let Range { mut start, end } = data.as_mut_ptr_range();

    let init_data = InitData(registry);

    let mut refs = Vec::new();
    for c in &ns.actors {
        let actor = init_data.0.actors.get(c.typename.as_ref()).unwrap();
        let addr = start.wrapping_add(start.align_offset(actor.align));
        start = addr.wrapping_add(actor.size);
        assert!(start <= end);
        let dest = unsafe { std::slice::from_raw_parts_mut(addr, actor.size) };
        refs.push((addr, *actor));
        (actor.constructor)(&init_data, dest).unwrap();
    }

    let main_data = MainData(init_data.0);

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

#[cfg(test)]
mod tests {
    use super::compute_space_bound;

    #[test]
    fn space_bound1() {
        let res = compute_space_bound([(4, 1), (2, 2), (2, 2)]);
        assert_eq!(res, 9);
    }

    #[test]
    fn space_bound2() {
        let res = compute_space_bound([(4, 4), (8, 8), (4, 4)]);
        assert_eq!(res, 23);
    }
}
