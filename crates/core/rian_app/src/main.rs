use std::{collections::HashMap, path::Path};

use rian::{
    config::{ActorConfig, Scope},
    serde_yaml, ContextId,
};

fn main() {
    let config = rian_app::Config {
        rian: rian::Config {
            contexts: vec![],
            root: Scope {
                name: None,
                children: HashMap::default(),
                actors: vec![
                    ActorConfig {
                        typename: "Foo".into(),
                        config: serde_yaml::Value::String("foo_config".into()),
                        context: ContextId::from_u32(0),
                    },
                    ActorConfig {
                        typename: "Bar".into(),
                        config: serde_yaml::Value::Null,
                        context: ContextId::from_u32(0),
                    },
                ],
                imported_scopes: vec![],
            },
        },
        shared_lib_paths: vec![
            Path::new("/home/andrew/rian/target/debug/liblib1.so").into(),
            Path::new("/home/andrew/rian/target/debug/liblib2.so").into(),
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

    rian::app::run(&config.rian);
}
