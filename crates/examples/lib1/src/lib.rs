use std::sync::Arc;
use std::time::Duration;

use rian::lookup::{AcyclicLocalKey, BroadcastGroup};
use rian::{register_actor, Accessor, Actor, InitArgs, MainArgs, UniquelyNamed};

use rian::CommonTrait;

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
    common_trait_obj: AcyclicLocalKey<dyn CommonTrait>,
    foo2: AcyclicLocalKey<Foo2>,
    foo2a: AcyclicLocalKey<Foo2>,
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
        let mut acc = args.accessor();
        args.spawn(async move {
            acc.send(|args, this| this.first_callback(args)).unwrap();
        });
        args.spawn(start_loop(args.accessor()));
        let foo = args.query().into();
        let foo2 = args.query().into();
        let foo2a = args.query().into();
        Ok(Self {
            s,
            common_trait_obj: foo,
            main_args: None,
            foo2,
            foo2a,
        })
    }
}

impl Foo {
    fn first_callback(&mut self, mut args: MainArgs) {
        let common = self.common_trait_obj.borrow_mut(&mut args);
        common.print_self();

        /*
         * Should fail to compile
        self.foo2.call(&mut args, |args, foo2| {
            let foo2a = self.foo2a.borrow_mut(args);
            println!(
                "Uh oh: {} {}",
                foo2 as *mut _ as usize, foo2a as *mut _ as usize
            );
        });
        */
    }
}

async fn start_loop(mut accessor: Accessor<Foo>) {
    for _ in 0..5 {
        rian::tokio::time::sleep(Duration::from_secs(1)).await;
        accessor
            .send(|args, this| this.first_callback(args))
            .unwrap();
    }
}
