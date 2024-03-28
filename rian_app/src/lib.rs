use std::{path::Path, sync::Arc};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub rian: rian::Config,
    pub shared_lib_paths: Vec<Arc<Path>>,
}
