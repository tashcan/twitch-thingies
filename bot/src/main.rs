mod settings;
use settings::*;

use futures_util::StreamExt;
use irc::client::prelude::*;

use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
enum BotError {
    #[error("Malformed Twitch PRIVMSG, missing required field `{0}`")]
    PrivMsgMissingField(&'static str),

    #[error("Ripper the idiot was lazy")]
    NotSpecified,
}

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
    badges: Vec<(String, i8)>,
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
            .map(|(name, value)| (name.to_owned(), value.parse::<i8>().unwrap()))
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
}

struct Tashbot {
    sender: tokio::sync::mpsc::UnboundedSender<TashControl>,
}

struct TashbotRunner {
    client: Client,
    control: tokio::sync::mpsc::UnboundedReceiver<TashControl>,
}

impl TashbotRunner {
    fn handle_message(&self, message: irc_proto::Message) {
        let sender = self.client.sender();
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                let twitch_msg = TwitchMessage::new(msg.clone(), message.tags.unwrap_or_default());
                match twitch_msg {
                    Ok(twitch_msg) => {
                        if twitch_msg.text.starts_with("!tash") {
                            sender.send_privmsg(target, "YAS! Valley Girl!").unwrap();
                        } else if twitch_msg.text.starts_with("!lurk") {
                            if let Err(e) = sender.send_privmsg(
                                target,
                                format!(
                                    "{} is now hiding in the shadows...",
                                    twitch_msg
                                        .display_name
                                        .as_ref()
                                        .map(|v| v.as_str())
                                        .unwrap_or("")
                                ),
                            ) {
                                error!("Failed to send reply {e}");
                            }
                        } else if twitch_msg.text.starts_with("!ladder") {
                            sender.send_privmsg(target, "It moves. Watch out.");
                        }
                    }
                    Err(e) => {
                        println!("{}", e);
                        //
                    }
                }
            }
            _ => print!("{}", message),
        }
    }

    fn handle_control(&self, control: TashControl) {
        println!("{:?}", control);
        match control {
            TashControl::JoinChannel(channel) => {
                self.client.send_join(channel).unwrap();
            }
            TashControl::LeaveChannel(channel) => {
                self.client.send_part(channel);
            }
        }
    }

    async fn run(mut self) {
        let mut stream = self.client.stream().unwrap();

        loop {
            tokio::select! {
                Some(message) = stream.next() => {
                    match message {
                        Ok(message) => {
                            self.handle_message(message)
                        }
                        Err(e) => {
                            // TODO: actually do nice error logging or something
                            println!("Invalid {}", e);
                        }
                    }
                }
                Some(control) = self.control.recv() => {
                    self.handle_control(control);
                }
            }
        }
    }
}

impl Tashbot {
    pub async fn new(nickname: &str, token: String) -> Self {
        let config = Config {
            nickname: Some(nickname.to_owned()),
            password: Some(format!("oauth:{}", token)),
            server: Some("irc.chat.twitch.tv".to_owned()),
            ..Config::default()
        };

        let client = Client::from_config(config).await.unwrap();
        client.identify().unwrap();
        client.send_cap_req(&[
            Capability::Custom("twitch.tv/commands"),
            Capability::Custom("twitch.tv/tags"),
        ]);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let runner = TashbotRunner {
            client,
            control: rx,
        };

        tokio::spawn(async move { runner.run().await });

        Self { sender: tx }
    }

    pub async fn join(&self, channel: &str) {
        self.sender
            .send(TashControl::JoinChannel(channel.to_owned()));
    }
}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    let settings = Settings::new().unwrap();

    let bot = Tashbot::new("heroictashbot", settings.bot_token).await;
    bot.join("#heroictashcan").await;

    loop {}

    Ok(())
}
