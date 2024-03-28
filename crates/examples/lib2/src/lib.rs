use rian::{register_actor, Actor, CommonTrait, InitStage, UniquelyNamed};

#[derive(UniquelyNamed)]
struct Bar {
    s: &'static str,
}

impl CommonTrait for Bar {}

register_actor!(Bar {
    dyn CommonTrait,
});

impl Actor for Bar {
    type Config = ();

    fn instantiate(data: &InitStage, config: ()) -> anyhow::Result<Self> {
        Ok(Self { s: "Success 2" })
    }
}
