use std::{
    collections::HashMap,
    path::Path,
    sync::{mpsc, Arc},
};

use common::{
    rian::{
        self,
        config::{ActorConfig, Context, Scope},
        ContextId,
    },
    serde_value,
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

    rian::run(config.rian);
}
