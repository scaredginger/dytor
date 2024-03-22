use rian::{declare_rian_lib, register_actor, Actor};

struct Bar {
    s: &'static str,
}

register_actor!(Bar, ns);

impl Actor for Bar {
    type Config = ();

    fn instantiate(data: &rian::InitData, config: ()) -> anyhow::Result<Self> {
        Ok(Self { s: "Success 2" })
    }

    fn name() -> &'static str {
        "Bar"
    }

    fn run(
        &self,
        data: &rian::MainData,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send + Sync {
        async {
            println!("Running {}", self.s);
            Ok(())
        }
    }
}

declare_rian_lib!();
