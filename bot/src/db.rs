use std::collections::HashMap;

use mysql_async::prelude::*;

use crate::BotError;

pub(crate) struct TashbotDb {
    pool: mysql_async::Pool,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub id: i32,
    pub name: String,
    pub prefix: String,
    pub description: Option<String>,
    pub reply: String,
    pub user_cooldown: Option<i32>,
    pub global_cooldown: Option<i32>,
    pub permissionbits: Option<u64>,
    pub enabled: Option<bool>,
    pub channel: i32,
}

impl TashbotDb {
    pub fn new(pool: mysql_async::Pool) -> Self {
        Self { pool }
    }

    pub async fn get_channels(&self) -> Result<HashMap<i32, String>, BotError> {
        let mut conn = self.pool.get_conn().await?;
        Ok("SELECT id, name FROM channels"
            .with(())
            .map(&mut conn, |(id, name)| (id, name))
            .await?
            .into_iter()
            .collect())
    }

    pub async fn get_commands(&self, channel: i32) -> Result<Vec<Command>, BotError> {
        let mut conn = self.pool.get_conn().await?;
        Ok("SELECT * FROM commands WHERE channel = :channel"
            .with(params! {
               "channel" => channel
            })
            .map(
                &mut conn,
                |(
                    id,
                    name,
                    prefix,
                    description,
                    reply,
                    user_cooldown,
                    global_cooldown,
                    permissionbits,
                    enabled,
                    channel,
                )| Command {
                    id,
                    name,
                    prefix,
                    description,
                    reply,
                    user_cooldown,
                    global_cooldown,
                    permissionbits,
                    enabled,
                    channel,
                },
            )
            .await?)
    }
}
