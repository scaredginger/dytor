use std::{collections::HashMap, ffi::CString};

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
    shared_lib_paths: Vec<CString>,
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
        shared_lib_paths: vec![CString::new(
            "target/x86_64-unknown-linux-gnu/debug/libreplay_mock.so",
        )
        .unwrap()],
    };

    let libs: Vec<_> = config
        .shared_lib_paths
        .iter()
        .map(|filename| {
            let name = filename.as_ptr();
            unsafe {
                libc::dlopen(
                    name,
                    libc::RTLD_LOCAL | libc::RTLD_NODELETE | libc::RTLD_LAZY,
                )
            }
        })
        .collect();

    rian::run(config.rian);

    for lib in libs {
        unsafe {
            libc::dlclose(lib);
        }
    }
}
