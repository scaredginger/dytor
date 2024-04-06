use std::{collections::HashMap, path::Path};

use rian::{
    config::{ActorConfig, Context, Scope},
    serde_yaml, tokio, ContextId,
};

fn main() {
    let config = rian_app::Config {
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
                        config: serde_yaml::Value::String("foo_config".into()),
                        context: ContextId::new(1).unwrap(),
                    },
                    ActorConfig {
                        typename: "Bar".into(),
                        config: serde_yaml::Value::Null,
                        context: ContextId::new(1).unwrap(),
                    },
                ],
                imported_scopes: vec![],
            },
        },
        shared_lib_paths: vec![
            Path::new("/home/andrew/rian/target/x86_64-unknown-linux-gnu/debug/liblib1.so").into(),
            Path::new("/home/andrew/rian/target/x86_64-unknown-linux-gnu/debug/liblib2.so").into(),
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

    rian::run(config.rian, rt.handle());
}
