use std::{collections::HashMap, mem, path::Path, sync::Arc};

use common::{
    rian::{
        self,
        config::{ActorConfig, Context, Scope},
        ContextId,
    },
    serde_value,
};

use common::serde::Deserialize;
use serde_value::Value as SerdeValue;

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
                        typename: "TokioSingleThread".into(),
                        config: SerdeValue::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "Synchronizer".into(),
                        config: SerdeValue::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "IntervalUnitProducer".into(),
                        config: SerdeValue::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "IntervalUnitConsumer".into(),
                        config: SerdeValue::Unit,
                        context: ContextId::new(1).unwrap(),
                    },
                ],
                imported_scopes: vec![],
            },
        },
        shared_lib_paths: vec![Path::new(
            "target/x86_64-unknown-linux-gnu/debug/libreplay_mock.so",
        )
        .into()],
    };

    for p in &config.shared_lib_paths {
        let s = p.as_os_str();
        let lib = unsafe { libloading::Library::new(s) }.unwrap();
        mem::forget(lib);
    }

    rian::run(config.rian);
}
