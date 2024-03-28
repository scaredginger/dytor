use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;

use crate::ContextId;

#[derive(Deserialize)]
pub struct ActorConfig {
    pub typename: Arc<str>,
    pub config: serde_yaml::Value,
    // pub context: ContextId,
}

pub struct NamespacePath(Vec<Arc<str>>);

impl<'de> Deserialize<'de> for NamespacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        Ok(Self(s.split("::").map(Arc::from).collect()))
    }
}

#[derive(Deserialize)]
pub struct Namespace {
    pub children: HashMap<Arc<str>, Namespace>,
    pub actors: Vec<ActorConfig>,
    pub namespace_imports: Vec<NamespacePath>,
}

#[derive(Deserialize)]
pub struct Context {
    pub id: ContextId,
    pub thread_affinity: Option<Vec<usize>>,
}

#[derive(Deserialize)]
pub struct Config {
    pub root_namespace: Namespace,
    pub contexts: Vec<Context>,
}
