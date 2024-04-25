use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;

use crate::context::ContextId;

#[derive(Deserialize)]
pub struct ActorConfig {
    pub typename: Arc<str>,
    pub config: serde_value::Value,
    pub context: ContextId,
}

#[derive(Deserialize)]
pub struct Scope {
    pub name: Option<Arc<str>>,
    pub children: HashMap<Arc<str>, Scope>,
    pub actors: Vec<ActorConfig>,
    pub imported_scopes: Vec<Arc<str>>,
}

#[derive(Deserialize)]
pub struct Context {
    pub id: ContextId,
    pub thread_affinity: Option<Vec<usize>>,
}

#[derive(Deserialize)]
pub struct Config {
    pub root: Scope,
    pub contexts: Vec<Context>,
}
