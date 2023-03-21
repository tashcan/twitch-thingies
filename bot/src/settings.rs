use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct Settings {
    pub bot_token: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let _run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let s = Config::builder()
            .add_source(File::with_name(".env.toml"))
            .add_source(Environment::with_prefix("tashbot"))
            .build()?;
        s.try_deserialize()
    }
}
