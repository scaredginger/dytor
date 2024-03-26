use std::{collections::HashMap, path::Path, sync::Arc};

use rian::serde_yaml;
use serde::Deserialize;

pub mod app;
pub mod arena;

#[derive(Deserialize)]
pub struct ActorConfig {
    pub typename: Arc<str>,
    pub config: serde_yaml::Value,
}

pub struct NamespacePath(Vec<Arc<str>>);

impl<'de> Deserialize<'de> for NamespacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        Ok(Self(s.split("::").map(|x| Arc::from(x)).collect()))
    }
}

#[derive(Deserialize)]
pub struct Namespace {
    pub children: HashMap<Arc<str>, Namespace>,
    pub actors: Vec<ActorConfig>,
    pub namespace_imports: Vec<NamespacePath>,
}

#[derive(Deserialize)]
pub struct Config {
    pub root_namespace: Namespace,
    pub shared_lib_paths: Vec<Arc<Path>>,
}
