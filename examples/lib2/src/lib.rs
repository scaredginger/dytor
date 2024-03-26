use rian::{register_actor, Actor, InitData, MainData};

struct Bar {
    s: &'static str,
}

register_actor!(Bar);

impl Actor for Bar {
    type Config = ();

    fn instantiate(data: &InitData, config: ()) -> anyhow::Result<Self> {
        Ok(Self { s: "Success 2" })
    }

    fn name() -> &'static str {
        "Bar"
    }

    fn run(
        &self,
        data: &MainData,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send + Sync {
        async {
            println!("Running {}", self.s);
            Ok(())
        }
    }
}
