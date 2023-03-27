mod settings;
use std::collections::HashMap;

use db::TashbotDb;
use runner::TashbotRunner;
use settings::*;

use irc::client::prelude::*;

use tokio::sync::mpsc::{self, error::SendError};

mod db;
mod error;
mod runner;
use error::*;

pub(crate) use db::Command;

#[derive(Debug)]
struct TwitchMessage {
    id: String,
    user_id: String,
    moderator: bool,
    subscriber: bool,
    vip: bool,
    color: String,
    display_name: Option<String>,
    bits: u32,
    badges: Vec<(String, u32)>,
    sub_months: u16,
    text: String,
}

fn get_tag_value(
    tags: &Vec<irc_proto::message::Tag>,
    tag_name: &'static str,
) -> Result<String, BotError> {
    tags.iter()
        .find(|tag| tag.0 == tag_name)
        .ok_or(BotError::PrivMsgMissingField(tag_name))
        .map(|tag| tag.1.as_ref().map(|v| v.as_str()).unwrap_or("").to_owned())
}

impl TwitchMessage {
    pub fn new(text: String, tags: Vec<irc_proto::message::Tag>) -> Result<Self, BotError> {
        let id = get_tag_value(&tags, "id")?;
        let user_id = get_tag_value(&tags, "user-id")?;
        let moderator = get_tag_value(&tags, "mod").map(|value| value == "1")?;
        let vip = get_tag_value(&tags, "vip").map(|_| true).unwrap_or(false);
        let subscriber = get_tag_value(&tags, "subscriber").map(|value| value == "1")?;
        let color = get_tag_value(&tags, "color")?;
        let display_name = get_tag_value(&tags, "display-name").ok();
        let bits = get_tag_value(&tags, "bits")
            .map(|value| value.parse::<u32>().unwrap())
            .unwrap_or(0);

        let badge_info = get_tag_value(&tags, "badge-info")?;
        let badges = get_tag_value(&tags, "badges")?;

        let badge_info: Vec<_> = badge_info
            .split(",")
            .filter_map(|info| {
                let mut splits = info.split("/");
                splits
                    .nth(0)
                    .map(|value| splits.nth(0).map(|value2| (value, value2)))
                    .flatten()
            })
            .collect();

        let badges: Vec<_> = badges
            .split(",")
            .filter_map(|info| {
                let mut splits = info.split("/");
                splits
                    .nth(0)
                    .map(|value| splits.nth(0).map(|value2| (value, value2)))
                    .flatten()
            })
            .map(|(name, value)| (name.to_owned(), value.parse::<u32>().unwrap()))
            .collect();

        Ok(Self {
            id,
            user_id,
            moderator,
            subscriber,
            vip,
            color,
            display_name,
            bits,
            badges,
            sub_months: badge_info
                .iter()
                .find(|(name, _)| name == &"subscriber")
                .map(|(_, value)| value.parse::<u16>().unwrap())
                .unwrap_or(0),
            text,
        })
    }
}

#[derive(Debug)]
enum TashControl {
    JoinChannel(String),
    LeaveChannel(String),
    UpdateCommand((String, db::Command)),
    RemoveCommand((String, i32)),
    Shutdown,
}

struct Tashbot {
    sender: mpsc::UnboundedSender<TashControl>,
    runner_handle: tokio::task::JoinHandle<()>,
    db: TashbotDb,
}

impl Tashbot {
    pub async fn new(nickname: &str, token: &str, pool: mysql_async::Pool) -> Self {
        let config = Config {
            nickname: Some(nickname.to_owned()),
            password: Some(format!("oauth:{}", token)),
            server: Some("irc.chat.twitch.tv".to_owned()),
            ..Config::default()
        };

        let client = Client::from_config(config).await.unwrap();
        client.identify().unwrap();
        let _ = client.send_cap_req(&[
            Capability::Custom("twitch.tv/commands"),
            Capability::Custom("twitch.tv/tags"),
        ]);

        let (tx, rx) = mpsc::unbounded_channel();
        let runner_handle = tokio::spawn(async move { TashbotRunner::new(client, rx).run().await });
        let db = TashbotDb::new(pool);

        Self {
            sender: tx,
            runner_handle,
            db,
        }
    }

    pub async fn join(&self, channel: &str) -> Result<(), BotError> {
        self.sender
            .send(TashControl::JoinChannel(channel.to_owned()))
            .map_err(|e| e.into())
    }

    pub async fn leave(&self, channel: &str) -> Result<(), BotError> {
        self.sender
            .send(TashControl::LeaveChannel(channel.to_owned()))
            .map_err(|e| e.into())
    }

    pub async fn update_command(
        &self,
        channel: &str,
        command: db::Command,
    ) -> Result<(), BotError> {
        self.sender
            .send(TashControl::UpdateCommand((channel.to_owned(), command)))
            .map_err(|e| e.into())
    }

    pub async fn join_channels(&self) -> Result<(), BotError> {
        for (_, channel) in self.db.get_channels().await? {
            self.join(&format!("#{}", channel)).await?;
        }
        Ok(())
    }

    pub async fn load_commands(&self) -> Result<(), BotError> {
        for (channel_id, channel_name) in self.db.get_channels().await? {
            for command in self.db.get_commands(channel_id).await? {
                self.update_command(&channel_name, command).await?;
            }
        }
        Ok(())
    }

    pub async fn shutdown(self) -> Result<(), BotError> {
        self.sender.send(TashControl::Shutdown)?;
        self.runner_handle.await.map_err(|e| e.into())
    }
}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    let settings = Settings::new()?;

    let builder = mysql_async::OptsBuilder::from_opts(
        mysql_async::Opts::from_url(&settings.database_url).unwrap(),
    );
    let pool = mysql_async::Pool::new(builder.ssl_opts(mysql_async::SslOpts::default()));

    let bot = Tashbot::new("heroictashbot", &settings.bot_token, pool.clone()).await;
    bot.join_channels().await?;
    bot.load_commands().await?;

    shutdown_signal().await;

    bot.shutdown().await?;

    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}
