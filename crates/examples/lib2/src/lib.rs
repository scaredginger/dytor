use rian::{register_actor, Actor, CommonTrait, InitStage, UniquelyNamed};

#[derive(UniquelyNamed, Debug)]
pub struct Bar {
    s: &'static str,
}

register_actor!(Bar {
    dyn CommonTrait,
});

impl Actor for Bar {
    type Config = ();

    fn instantiate(data: &mut InitStage, config: ()) -> anyhow::Result<Self> {
        Ok(Self { s: "Success 2" })
    }
}
