use super::world::Player;
use super::{Frame, Identity, Kind};
use crate::entity::punishment;
use crate::error::ClientError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Protocol {
    V1,
    V2,
}

/// A clientbound packet
#[derive(Serialize, Clone)]
#[serde(tag = "m", content = "c")]
pub enum ClientPacket {
    MojangInfo {
        session_hash: String,
    },
    NewJWT {
        token: String,
    },
    Message {
        author_info: UserInfo,
        content: String,
    },
    PrivateMessage {
        author_info: UserInfo,
        content: String,
    },
    UserCount {
        connections: u32,
        logged_in: u32,
    },
    Success {
        reason: SuccessReason,
    },
    Error {
        message: ClientError,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Hello {
        protocol: u32,
    },
    Punished {
        kind: punishment::Kind,
        reason: String,
        expires: Option<i64>,
    },
    Punishments {
        user: UserRef,
        punishments: Vec<PunishmentView>,
    },
    Welcome {
        user: Author,
        staff: bool,
    },
    Settings(SettingsView),
    ChatMessage {
        channel: String,
        id: u64,
        time: i64,
        author: Author,
        content: String,
    },
    Reports {
        reports: Vec<ReportView>,
    },
    ReportCreated {
        report: ReportView,
    },
}

#[derive(Serialize, Clone)]
pub struct ReportView {
    pub id: Uuid,
    pub reporter: UserRef,
    pub target: UserRef,
    pub channel: Option<String>,
    pub message: Option<u64>,
    pub content: Option<String>,
    pub reason: String,
    pub time: i64,
}

#[derive(Serialize, Clone)]
pub struct Author {
    #[serde(flatten)]
    pub user: UserRef,
    pub roles: Vec<RoleView>,
    pub highlight: bool,
}

#[derive(Serialize, Clone)]
pub struct RoleView {
    pub id: String,
    pub name: String,
    pub staff: bool,
}

#[derive(Serialize, Clone)]
pub struct SettingsView {
    pub allow_messages: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_servers: Option<bool>,
    pub server_chat: bool,
}

#[derive(Serialize, Clone)]
pub struct UserRef {
    pub id: Uuid,
    pub kind: Kind,
    pub name: String,
    pub uuid: Uuid,
    /// The Minecraft account an online LiquidBounce Account proved it plays on.
    pub minecraft: Option<Player>,
}

impl From<&Identity> for UserRef {
    fn from(identity: &Identity) -> UserRef {
        UserRef {
            id: identity.id,
            kind: identity.kind,
            name: identity.name.clone(),
            uuid: identity.uuid,
            minecraft: None,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct PunishmentView {
    pub id: Uuid,
    pub kind: punishment::Kind,
    pub ip: Option<String>,
    pub reason: String,
    pub issued_by: Option<UserRef>,
    pub created: i64,
    pub expires: Option<i64>,
}

impl ClientPacket {
    pub fn error(error: ClientError) -> ClientPacket {
        ClientPacket::Error {
            detail: error.detail(),
            message: error,
        }
    }

    pub fn error_with(error: ClientError, detail: impl Into<String>) -> ClientPacket {
        ClientPacket::Error {
            message: error,
            detail: Some(detail.into()),
        }
    }

    pub fn encode(&self, protocol: Protocol) -> Frame {
        let json = match self {
            ClientPacket::Error { message, detail: Some(_) } if protocol == Protocol::V1 => {
                serde_json::to_string(&ClientPacket::Error {
                    message: message.clone(),
                    detail: None,
                })
            }
            packet => serde_json::to_string(packet),
        };
        Frame(json.expect("could not encode packet").into())
    }
}

/// A serverbound packet
#[derive(Deserialize)]
#[serde(tag = "m", content = "c")]
pub enum ServerPacket {
    RequestMojangInfo,
    LoginMojang(User),
    LoginJWT { token: String, allow_messages: bool },
    LoginAccount { token: String, allow_messages: bool },
    RequestJWT,
    Message { content: String },
    PrivateMessage { receiver: String, content: String },
    BanUser { user: Uuid },
    UnbanUser { user: Uuid },
    RequestUserCount,
    Hello { protocol: u32 },
    Punish {
        user: Option<String>,
        ip: Option<String>,
        kind: punishment::Kind,
        /// Seconds; `None` is permanent.
        duration: Option<u64>,
        reason: String,
        #[serde(default)]
        include_ip: bool,
    },
    Pardon { user: Option<String>, ip: Option<String> },
    RequestPunishments { user: String },
    Settings {
        allow_messages: Option<bool>,
        watch_servers: Option<bool>,
        server_chat: Option<bool>,
    },
    ChatMessage { channel: String, content: String },
    Report { user: String, message: Option<u64>, reason: String },
    RequestReports,
    ResolveReport { id: Uuid },
    Location { server: Option<String> },
}

/// The author of a v1 message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub uuid: Uuid,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub name: String,
    pub uuid: Uuid,
    /// Should this user allow private messages?
    pub allow_messages: bool,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum SuccessReason {
    Login,
    Ban,
    Unban,
    Punish,
    Pardon,
    Report,
    Resolve,
    /// A LiquidBounce Account session proved the Minecraft account it plays on.
    Minecraft,
}

#[cfg(test)]
mod tests {
    use super::{ClientError, ClientPacket, Protocol};

    // the object form `{"InvalidCharacter":"§"}` made old clients drop the connection
    #[test]
    fn v1_errors_are_plain_codes() {
        let frame = ClientPacket::error(ClientError::InvalidCharacter('§')).encode(Protocol::V1);
        assert_eq!(frame.0, r#"{"m":"Error","c":{"message":"InvalidCharacter"}}"#);
    }
}
