use std::{collections::HashMap, path::Path};

#[tokio::main]
async fn main() {
    let config = rian::app::Config {
        root_namespace: rian::app::Namespace {
            children: HashMap::default(),
            actors: vec![
                rian::app::ActorConfig {
                    typename: "Foo".into(),
                    config: serde_yaml::Value::Null,
                },
                rian::app::ActorConfig {
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
    rian::main(&config).await;
}
