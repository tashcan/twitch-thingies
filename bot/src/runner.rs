use futures_util::StreamExt;
use irc::client::Client;
use irc_proto::Command;
use tracing::error;

use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::{TashControl, TwitchMessage};

pub(crate) struct TashbotRunner {
    client: Client,
    control: mpsc::UnboundedReceiver<TashControl>,
    commands: HashMap<String, Vec<crate::Command>>,
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
            commands: HashMap::new(),
        }
    }

    fn handle_message(&self, message: irc_proto::Message) {
        let sender = self.client.sender();
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                let twitch_msg = TwitchMessage::new(msg.clone(), message.tags.unwrap_or_default());
                match twitch_msg {
                    Ok(twitch_msg) => {
                        let target_stripped = &target[1..];
                        if let Some(commands_for_channel) = self.commands.get(target_stripped) {
                            if let Some(command) = commands_for_channel
                                .iter()
                                .find(|command| twitch_msg.text.starts_with(&command.prefix))
                            {
                                if let Err(e) = sender.send_privmsg(
                                    target,
                                    message_format(&command.reply, &twitch_msg),
                                ) {
                                    error!("Failed to send reply {e}");
                                }
                                //
                            }
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

    fn handle_control(&mut self, control: TashControl) -> bool {
        match control {
            TashControl::Shutdown => return false,
            TashControl::JoinChannel(channel) => {
                self.client.send_join(channel).unwrap();
            }
            TashControl::LeaveChannel(channel) => {
                let _ = self.client.send_part(channel);
            }
            TashControl::UpdateCommand((channel, command)) => {
                let commands_for_channel = self.commands.entry(channel).or_insert_with(Vec::new);
                if let Some(cmd) = commands_for_channel
                    .iter_mut()
                    .find(|cmd| cmd.id == command.id)
                {
                    *cmd = command;
                } else {
                    commands_for_channel.push(command);
                }
            }
            TashControl::RemoveCommand((channel, command_id)) => {
                //
            }
        }
        true
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
                    if !self.handle_control(control) {
                        break;
                    }
                }
            }
        }
    }
}
