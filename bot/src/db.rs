use mysql_async::prelude::*;

use crate::BotError;

pub(crate) struct TashbotDb {
    pool: mysql_async::Pool,
}

impl TashbotDb {
    pub fn new(pool: mysql_async::Pool) -> Self {
        Self { pool }
    }

    pub async fn get_channels(&self) -> Result<Vec<String>, BotError> {
        let mut conn = self.pool.get_conn().await?;
        Ok("SELECT name FROM channels"
            .with(())
            .map(&mut conn, |name| name)
            .await?)
    }
}
