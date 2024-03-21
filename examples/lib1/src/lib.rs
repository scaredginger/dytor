use rian::{declare_rian_lib, register_actor, Actor};

struct Foo {
    s: &'static str,
}

register_actor!(Foo, ns);

impl Actor for Foo {
    type Config<'de> = ();

    fn instantiate(data: &rian::InitData) -> anyhow::Result<Self> {
        Ok(Self { s: "Success" })
    }

    fn name() -> &'static str {
        "Foo"
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
