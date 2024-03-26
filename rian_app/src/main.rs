use std::{collections::HashMap, path::Path};

use rian::{serde_yaml, tokio};

#[tokio::main]
async fn main() {
    let config = rian_app::Config {
        root_namespace: rian_app::Namespace {
            children: HashMap::default(),
            actors: vec![
                rian_app::ActorConfig {
                    typename: "Foo".into(),
                    config: serde_yaml::Value::String("foo_config".into()),
                },
                rian_app::ActorConfig {
                    typename: "Bar".into(),
                    config: serde_yaml::Value::Null,
                },
            ],
            namespace_imports: vec![],
        },
        shared_lib_paths: vec![
            Path::new("/home/andrew/rian/target/debug/liblib1.so").into(),
            Path::new("/home/andrew/rian/target/debug/liblib2.so").into(),
        ],
    };
    rian_app::app::main(&config).await;
}
