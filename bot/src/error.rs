use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

use crate::TashControl;

#[derive(Error, Debug)]
pub(crate) enum BotError {
    #[error("Malformed Twitch PRIVMSG, missing required field `{0}`")]
    PrivMsgMissingField(&'static str),

    #[error("Internal communication error. Runner no longer exists.")]
    InternalCommmunicationErrorRunnerDead(#[from] SendError<TashControl>),

    #[error("Invalid configuration")]
    InvalidConfiguration(#[from] config::ConfigError),

    #[error("Database error")]
    DatabaseError(#[from] mysql_async::Error),

    #[error("Join Error")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Ripper the idiot was lazy")]
    NotSpecified,
}
