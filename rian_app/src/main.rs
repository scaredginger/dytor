use std::{collections::HashMap, path::Path};

use rian::{
    config::{ActorConfig, Namespace},
    serde_yaml,
};

fn main() {
    let config = rian_app::Config {
        rian: rian::Config {
            contexts: vec![],
            root_namespace: Namespace {
                children: HashMap::default(),
                actors: vec![
                    ActorConfig {
                        typename: "Foo".into(),
                        config: serde_yaml::Value::String("foo_config".into()),
                    },
                    ActorConfig {
                        typename: "Bar".into(),
                        config: serde_yaml::Value::Null,
                    },
                ],
                namespace_imports: vec![],
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
