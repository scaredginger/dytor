use std::sync::Arc;

use rian::lookup::BroadcastGroup;
use rian::{register_actor, Actor, InitStage, UniquelyNamed};

use rian::CommonTrait;

#[derive(UniquelyNamed, Debug)]
pub struct Foo {
    s: Arc<str>,
}

trait ExampleTrait {}
impl ExampleTrait for Foo {}

register_actor!(Foo {
    dyn CommonTrait,
    dyn ExampleTrait,
});

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(data: &mut InitStage, s: Arc<str>) -> anyhow::Result<Self> {
        let common: BroadcastGroup<dyn CommonTrait> = data.query().into();
        data.broadcast(common, |_, t| {
            t.print_self();
        });
        Ok(Self { s })
    }
}
