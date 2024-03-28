use std::{sync::Arc, time::Duration};

use rian::{register_actor, uniquely_named, Actor, InitStage, MainStage, UniquelyNamed};

use rian::CommonTrait;

#[derive(UniquelyNamed)]
struct Foo {
    s: Arc<str>,
}

impl CommonTrait for Foo {}

register_actor!(Foo {
    dyn CommonTrait,
});

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(data: &InitStage, s: Arc<str>) -> anyhow::Result<Self> {
        println!("Foo {}", data.request::<Foo>().count());
        println!(
            "CommonTrait {}",
            data.request_dyn::<dyn CommonTrait>().count()
        );
        Ok(Self { s })
    }
}
