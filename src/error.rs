use serde::Serialize;
use snafu::Snafu;
use std::{error, fmt, io};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("I/O: {}", source))]
    IO { source: io::Error },
    #[snafu(display("JSON: {}", source))]
    JSON { source: serde_json::error::Error },
    #[snafu(display("actix-web: {}", source))]
    Actix { source: actix_web::Error },
    #[cfg(feature = "openssl-tls")]
    #[snafu(display("OpenSSL: {}", source))]
    OpenSSL { source: openssl::error::ErrorStack },
    #[cfg(feature = "rustls-tls")]
    #[snafu(display("rustls: {}", source))]
    RustTLS { source: std::io::Error },
    #[cfg(feature = "rustls-tls")]
    #[snafu(display("rustls"))]
    RustTLSNoMsg,
    #[snafu(display("JWT: {}", source))]
    JWT { source: jsonwebtoken::errors::Error },
    #[snafu(display("database: {}", source))]
    Database { source: sea_orm::DbErr },
    #[snafu(display("actor: {}", source))]
    Mailbox { source: actix::MailboxError },
    #[snafu(display("UUID parsing: {}", source))]
    Uuid { source: uuid::Error },
    #[snafu(display("axochat: {}", source))]
    AxoChat { source: ClientError },
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::IO { source }
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(source: serde_json::error::Error) -> Self {
        Error::JSON { source }
    }
}

impl From<actix_web::Error> for Error {
    fn from(source: actix_web::Error) -> Self {
        Error::Actix { source }
    }
}

#[cfg(feature = "openssl-tls")]
impl From<openssl::error::ErrorStack> for Error {
    fn from(source: openssl::error::ErrorStack) -> Self {
        Error::OpenSSL { source }
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(source: jsonwebtoken::errors::Error) -> Self {
        Error::JWT { source }
    }
}

impl From<sea_orm::DbErr> for Error {
    fn from(source: sea_orm::DbErr) -> Self {
        Error::Database { source }
    }
}

impl From<actix::MailboxError> for Error {
    fn from(source: actix::MailboxError) -> Self {
        Error::Mailbox { source }
    }
}

impl From<uuid::Error> for Error {
    fn from(source: uuid::Error) -> Self {
        Error::Uuid { source }
    }
}

impl From<ClientError> for Error {
    fn from(source: ClientError) -> Self {
        Error::AxoChat { source }
    }
}

/// A client-facing error.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ClientError {
    NotSupported,
    LoginFailed,
    NotLoggedIn,
    AlreadyLoggedIn,
    MojangRequestMissing,
    NotPermitted,
    NotBanned,
    Banned,
    RateLimited,
    PrivateMessageNotAccepted,
    EmptyMessage,
    MessageTooLong,
    InvalidCharacter(char),
    InvalidId,
    AccountRequired,
    Muted,
    UnknownUser(String),
    UnknownChannel(String),
    InvalidPacket(String),
    Internal,
}

impl ClientError {
    /// The name of the error without its value.
    pub fn code(&self) -> &'static str {
        use self::ClientError::*;

        match self {
            NotSupported => "NotSupported",
            LoginFailed => "LoginFailed",
            NotLoggedIn => "NotLoggedIn",
            AlreadyLoggedIn => "AlreadyLoggedIn",
            MojangRequestMissing => "MojangRequestMissing",
            NotPermitted => "NotPermitted",
            NotBanned => "NotBanned",
            Banned => "Banned",
            RateLimited => "RateLimited",
            PrivateMessageNotAccepted => "PrivateMessageNotAccepted",
            EmptyMessage => "EmptyMessage",
            MessageTooLong => "MessageTooLong",
            InvalidCharacter(_) => "InvalidCharacter",
            InvalidId => "InvalidId",
            AccountRequired => "AccountRequired",
            Muted => "Muted",
            UnknownUser(_) => "UnknownUser",
            UnknownChannel(_) => "UnknownChannel",
            InvalidPacket(_) => "InvalidPacket",
            Internal => "Internal",
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            ClientError::InvalidCharacter(ch) => Some(ch.to_string()),
            ClientError::UnknownUser(detail) | ClientError::UnknownChannel(detail) | ClientError::InvalidPacket(detail) => Some(detail.clone()),
            _ => None,
        }
    }
}

impl error::Error for ClientError {}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ClientError::*;

        match self {
            NotSupported => write!(f, "method not supported"),
            LoginFailed => write!(f, "login failed"),
            NotLoggedIn => write!(f, "not logged in"),
            AlreadyLoggedIn => write!(f, "already logged in"),
            MojangRequestMissing => write!(f, "mojang request missing"),
            NotPermitted => write!(f, "not permitted"),
            NotBanned => write!(f, "not banned"),
            Banned => write!(f, "banned"),
            RateLimited => write!(f, "rate limited"),
            PrivateMessageNotAccepted => write!(f, "private message not accepted"),
            EmptyMessage => write!(f, "empty message"),
            MessageTooLong => write!(f, "message was too long"),
            InvalidCharacter(ch) => write!(
                f,
                "message contained invalid character: `{}`",
                ch.escape_default()
            ),
            InvalidId => write!(f, "invalid id"),
            AccountRequired => write!(f, "a LiquidBounce Account is required"),
            Muted => write!(f, "muted"),
            UnknownUser(user) => write!(f, "unknown user `{}`", user),
            UnknownChannel(channel) => write!(f, "unknown channel `{}`", channel),
            InvalidPacket(reason) => write!(f, "invalid packet: {}", reason),
            Internal => write!(f, "internal error"),
        }
    }
}
