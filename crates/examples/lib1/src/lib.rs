use std::sync::Arc;

use rian::lookup::{AcyclicLocalKey, BroadcastGroup};
use rian::{register_actor, Actor, InitArgs, UniquelyNamed};

use rian::CommonTrait;

#[derive(UniquelyNamed)]
pub struct Foo {
    s: Arc<str>,
    foo: AcyclicLocalKey<dyn CommonTrait>,
}

register_actor!(Foo);

impl Actor for Foo {
    type Config = Arc<str>;

    fn instantiate(mut args: InitArgs<Self>, s: Arc<str>) -> anyhow::Result<Self> {
        let common: BroadcastGroup<dyn CommonTrait> = args.query().into();
        args.broadcast(common, |_, t| {
            t.print_self();
        });
        let mut acc = args.accessor();
        args.tokio_handle().spawn(async move {
            acc.send(|ctx, this| {
                let x = ctx.get_mut(&mut this.foo);
                x.print_self();
            })
            .unwrap();
        });
        let foo = args.query().into();
        Ok(Self { s, foo })
    }
}
