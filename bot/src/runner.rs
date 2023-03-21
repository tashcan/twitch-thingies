use futures_util::StreamExt;
use irc::client::Client;
use irc_proto::Command;
use tracing::error;

use tokio::sync::mpsc;

use crate::{TashControl, TwitchMessage};

pub(crate) struct TashbotRunner {
    client: Client,
    control: mpsc::UnboundedReceiver<TashControl>,
}

fn message_format(fmt: &str, twitch_msg: &TwitchMessage) -> String {
    let sender_name = twitch_msg
        .display_name
        .as_ref()
        .map(|v| v.as_str())
        .unwrap_or("");
    let fmt = fmt.replace("{sender.name}", sender_name);
    format!("{fmt}")
}

impl TashbotRunner {
    pub fn new(client: Client, control_receiver: mpsc::UnboundedReceiver<TashControl>) -> Self {
        Self {
            client,
            control: control_receiver,
        }
    }

    fn handle_message(&self, message: irc_proto::Message) {
        let sender = self.client.sender();
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                println!("{target}");
                let twitch_msg = TwitchMessage::new(msg.clone(), message.tags.unwrap_or_default());
                match twitch_msg {
                    Ok(twitch_msg) => {
                        if twitch_msg.text.starts_with("!tash") {
                            sender.send_privmsg(target, "YAS! Valley Girl!").unwrap();
                        } else if twitch_msg.text.starts_with("!lurk") {
                            if let Err(e) = sender.send_privmsg(
                                target,
                                message_format(
                                    "{sender.name} is now hiding in the shadows...",
                                    &twitch_msg,
                                ),
                            ) {
                                error!("Failed to send reply {e}");
                            }
                        } else if twitch_msg.text.starts_with("!ladder") {
                            sender.send_privmsg(
                                target,
                                message_format("It moves. Watch out.", &twitch_msg),
                            );
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
        match control {
            TashControl::JoinChannel(channel) => {
                self.client.send_join(channel).unwrap();
            }
            TashControl::LeaveChannel(channel) => {
                let _ = self.client.send_part(channel);
            }
            TashControl::AddCommand {
                ref channel,
                ref cmd,
                ref msg,
            } => {
                //
            }
        }
    }

    pub async fn run(mut self) {
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
