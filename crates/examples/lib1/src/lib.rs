use std::sync::Arc;

use common::anyhow;
use common::rian::lookup::BroadcastGroup;
use common::rian::{register_actor, Actor, InitArgs, MainArgs, UniquelyNamed};

use common::CommonTrait;

#[derive(UniquelyNamed)]
pub struct Foo2 {
    a: u32,
}

register_actor!(Foo2);

impl Actor for Foo2 {
    type Config = ();

    fn init(args: InitArgs<Self>, config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self { a: 0 })
    }
}

#[derive(UniquelyNamed)]
pub struct Foo {
    s: Arc<str>,
    main_args: Option<MainArgs<'static>>,
}

register_actor!(Foo);

impl Actor for Foo {
    type Config = Arc<str>;

    fn init(mut args: InitArgs<Self>, s: Arc<str>) -> anyhow::Result<Self> {
        let common: BroadcastGroup<dyn CommonTrait> = args.query().into();
        args.broadcast(common, |_, t| {
            t.print_self();
        });
        Ok(Self { s, main_args: None })
    }
}
