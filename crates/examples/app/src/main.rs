use std::{collections::HashMap, path::Path, sync::Arc};

use common::{
    rian::{
        self,
        config::{ActorConfig, Context, Scope},
        ContextId,
    },
    serde_value,
    tokio::{self, select, sync::mpsc, task::JoinSet},
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    rian: rian::Config,
    shared_lib_paths: Vec<Arc<Path>>,
}

fn main() {
    let config = Config {
        rian: rian::Config {
            contexts: vec![Context {
                id: ContextId::new(1).unwrap(),
                thread_affinity: None,
            }],
            root: Scope {
                name: None,
                children: HashMap::default(),
                actors: vec![
                    ActorConfig {
                        typename: "Foo".into(),
                        config: serde_value::Value::String("foo_config".into()),
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "Bar".into(),
                        config: serde_value::Value::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "Foo2".into(),
                        config: serde_value::Value::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                ],
                imported_scopes: vec![],
            },
        },
        shared_lib_paths: vec![
            Path::new("target/x86_64-unknown-linux-gnu/debug/liblib1.so").into(),
            Path::new("target/x86_64-unknown-linux-gnu/debug/liblib2.so").into(),
        ],
    };

    let _libs: Vec<_> = config
        .shared_lib_paths
        .iter()
        .map(|p| {
            let s = p.as_os_str();
            unsafe { libloading::Library::new(s) }.unwrap()
        })
        .collect();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let spawn_fn = Arc::new(move |fut| tx.send(fut).unwrap());
    rt.spawn(async move {
        let mut js = JoinSet::new();
        loop {
            if js.is_empty() {
                let Some(fut) = rx.recv().await else {
                    break;
                };
                js.spawn(fut);
            }
            select! {
                _ = js.join_next() => {}
                 fut = rx.recv() => {
                     let Some(fut) = fut else {
                        break;
                     };
                     js.spawn(fut);
                 }
            }
        }
        while let Some(_) = js.join_next().await {}
    });

    rian::run(config.rian, spawn_fn);
}
