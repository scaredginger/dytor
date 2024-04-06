use std::sync::Arc;

use rian::lookup::BroadcastGroup;
use rian::{register_actor, Actor, InitArgs, UniquelyNamed};

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

    fn instantiate(mut args: InitArgs<Self>, s: Arc<str>) -> anyhow::Result<Self> {
        let common: BroadcastGroup<dyn CommonTrait> = args.query().into();
        args.broadcast(common, |_, t| {
            t.print_self();
        });
        let mut acc = args.accessor();
        args.tokio_handle().spawn(async move {
            acc.send(|_, _| {
                println!("Sent");
            })
            .unwrap();
        });
        Ok(Self { s })
    }
}
