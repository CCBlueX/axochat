use super::UserId;

use std::fmt;

/// Direct messages address the user by id or name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    Global,
    Server,
    /// A server channel as staff watching every server see it; never sent to.
    Watched(String),
    User(String),
}

impl Channel {
    pub fn parse(channel: &str) -> Option<Channel> {
        Some(match channel.split_once('/') {
            None if channel == "global" => Channel::Global,
            None if channel == "server" => Channel::Server,
            Some(("user", user)) if !user.is_empty() => Channel::User(user.to_owned()),
            _ => return None,
        })
    }

    pub fn direct(user: UserId) -> Channel {
        Channel::User(user.to_string())
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Channel::Global => f.write_str("global"),
            Channel::Server => f.write_str("server"),
            Channel::Watched(address) => write!(f, "server/{}", address),
            Channel::User(user) => write!(f, "user/{}", user),
        }
    }
}
