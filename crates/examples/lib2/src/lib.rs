use common::rian::{register_actor, Actor, InitArgs, UniquelyNamed};

use common::anyhow;
use common::CommonTrait;

#[derive(UniquelyNamed, Debug)]
pub struct Bar {
    s: &'static str,
}

register_actor!(Bar {
    dyn CommonTrait,
});

impl Actor for Bar {
    type Config = ();

    fn init(args: InitArgs<Self>, config: ()) -> anyhow::Result<Self> {
        Ok(Self { s: "Success 2" })
    }

    fn is_finished(&self) -> bool {
        true
    }

    fn stop(&mut self) {}
}
