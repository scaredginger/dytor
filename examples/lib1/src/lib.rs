use std::{sync::Arc, time::Duration};

use rian::{register_actor, Actor, InitData, MainData, tokio};

struct Foo {
    s: Arc<str>,
}

register_actor!(Foo);

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(data: &InitData, s: Arc<str>) -> anyhow::Result<Self> {
        Ok(Self { s })
    }

    fn name() -> &'static str {
        "Foo"
    }

    async fn run(&self, data: &MainData) -> anyhow::Result<()> {
        println!("Running {}", self.s);
        let handle = tokio::spawn(async {
            for i in 1..=10 {
                println!("print {i}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        println!("spawned task");
        handle.await.unwrap();
        Ok(())
    }
}
