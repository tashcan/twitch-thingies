use config::{Config};
use std::env;

pub struct Settings {
    pub bot_token: String;
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let _run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let s = Config::builder()
            .add_source(File::with_name(".env"))
            .add_source(Environment::with_prefix("tashbot"))
            .build()?;
        s.try_deserialize()
    }
}
