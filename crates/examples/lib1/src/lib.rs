use std::sync::Arc;

use rian::{register_actor, Actor, InitStage, UniquelyNamed};

use rian::CommonTrait;

#[derive(UniquelyNamed)]
struct Foo {
    s: Arc<str>,
}

impl CommonTrait for Foo {}

trait ExampleTrait {}
impl ExampleTrait for Foo {}

register_actor!(Foo {
    dyn CommonTrait,
    dyn ExampleTrait,
});

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(data: &InitStage, s: Arc<str>) -> anyhow::Result<Self> {
        println!("Foo {}", data.query::<Foo>().all_refs().count());
        //
        println!(
            "CommonTrait {}",
            data.query::<dyn CommonTrait>().all_refs().count()
        );
        Ok(Self { s })
    }
}
