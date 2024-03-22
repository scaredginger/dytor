use std::{sync::Arc, time::Duration};

use rian::{declare_rian_lib, register_actor, Actor};

use core_lib::tokio;

struct Foo {
    s: Arc<str>,
}

register_actor!(Foo, ns);

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(data: &rian::InitData, s: Arc<str>) -> anyhow::Result<Self> {
        Ok(Self { s })
    }

    fn name() -> &'static str {
        "Foo"
    }

    async fn run(&self, data: &rian::MainData) -> anyhow::Result<()> {
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

declare_rian_lib!();
