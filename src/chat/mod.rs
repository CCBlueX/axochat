mod channel;
mod connect;
mod handler;
mod history;
mod id;
mod session;

pub use id::*;

use channel::{Channel, Target};
use history::History;

use crate::config::Config;
use crate::error::*;
use log::*;

use actix::*;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::api::{Api, RoleDefinition};
use crate::auth::{Authenticator, UserInfo};
use crate::ip::RealIp;
use crate::message::{MessageValidator, RateLimiter};
use crate::moderation::Moderation;
use crate::store::{Persist, Store, Write};
use rand::{rngs::SysRng, SeedableRng};
use rand_hc::Hc128Rng;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const MAINTENANCE: Duration = Duration::from_secs(3600);
const ROLE_REFRESH: Duration = Duration::from_secs(600);
/// The protocol a client announces with `Hello` to get the packets added after the original ones.
pub const PROTOCOL: u32 = 2;

pub async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<ChatServer>>,
    real_ip: web::Data<RealIp>,
) -> actix_web::Result<HttpResponse> {
    let ip = real_ip.of(&req);
    let (response, ws, messages) = actix_ws::handle(&req, stream)?;
    session::Session::create(|ctx| {
        ctx.add_stream(messages);
        session::Session::new(InternalId::new(0), srv.get_ref().clone(), ws, ip)
    });
    Ok(response)
}

pub struct ChatServer {
    connections: HashMap<InternalId, SessionState>,
    /// Keyed by the id in the database.
    users: HashMap<Uuid, UserSession>,

    api: Arc<Api>,
    /// In the order of the Service API.
    roles: Vec<RoleDefinition>,
    rng: rand_hc::Hc128Rng,
    authenticator: Option<Authenticator>,
    validator: MessageValidator,
    moderation: Moderation,
    store: Addr<Store>,
    config: Config,
    history: History,

    current_internal_user_id: u64,
}

impl ChatServer {
    pub fn new(config: Config, store: Addr<Store>, moderation: Moderation) -> ChatServer {
        ChatServer {
            connections: HashMap::new(),
            users: HashMap::new(),

            api: Arc::new(Api::new(&config.api)),
            roles: Vec::new(),
            rng: Hc128Rng::try_from_rng(&mut SysRng).expect("could not initialize hc128 rng"),
            authenticator: Authenticator::new(&config.auth),
            validator: MessageValidator::new(config.message.clone()),
            moderation,
            store,
            config,
            history: History::new(),

            current_internal_user_id: 0,
        }
    }
}

// try_send would also fail on a full mailbox; only a closed one is a delivery failure.
pub(crate) fn send_message(recipient: &Recipient<ClientPacket>, message: ClientPacket, context: &str) -> bool {
    if recipient.connected() {
        recipient.do_send(message);
        true
    } else {
        warn!("Could not send {} to user: mailbox closed", context);
        false
    }
}

impl Actor for ChatServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        self.refresh_roles(ctx);
        ctx.run_interval(ROLE_REFRESH, |actor, ctx| actor.refresh_roles(ctx));
        ctx.run_interval(MAINTENANCE, |actor, _ctx| {
            let now = now_ms();
            actor.moderation.prune(now);
            let retention = actor.config.moderation.ip_retention.as_millis() as i64;
            actor.persist(vec![Write::ForgetIps { seen_before: now - retention }]);
        });
    }
}

impl ChatServer {
    fn persist(&self, writes: Vec<Write>) {
        self.store.do_send(Persist(writes));
    }

    fn send(&self, user_id: InternalId, packet: ClientPacket) {
        if let Some(session) = self.connections.get(&user_id) {
            send_message(&session.addr, packet, "packet");
        }
    }

    /// Clients that sent `Hello` get the error code with its value apart.
    fn send_error(&self, user_id: InternalId, error: ClientError) {
        let Some(session) = self.connections.get(&user_id) else { return };
        let packet = if session.hello {
            ClientPacket::ErrorDetail {
                message: error.code(),
                detail: error.detail(),
            }
        } else {
            ClientPacket::Error { message: error }
        };
        send_message(&session.addr, packet, "error");
    }

    /// Ends the login of a connection, which stays open.
    fn logout(&mut self, id: InternalId) {
        let Some(stored_id) = self.connections.get_mut(&id).and_then(|session| {
            session.user = None;
            session.minecraft = None;
            session.stored_id.take()
        }) else {
            return;
        };
        if let Some(user_session) = self.users.get_mut(&stored_id) {
            user_session.connections.remove(&id);
            if user_session.connections.is_empty() {
                self.users.remove(&stored_id);
            }
        }
    }

    fn refresh_roles(&mut self, ctx: &mut Context<Self>) {
        let api = self.api.clone();
        ctx.spawn(async move { api.roles().await }.into_actor(self).map(|result, actor, _ctx| match result {
            Ok(roles) => actor.roles = roles,
            Err(err) => warn!("Could not refresh roles: {}", err),
        }));
    }

    fn has_staff_role(&self, roles: &[String]) -> bool {
        roles.iter().any(|role| self.role(role).is_some_and(|role| role.is_staff))
    }

    fn role(&self, id: &str) -> Option<&RoleDefinition> {
        self.roles.iter().find(|role| role.id == id)
    }

    /// Whether the logged in user of a connection is staff.
    fn is_staff(&self, user_id: InternalId) -> bool {
        self.online_user(user_id)
            .is_some_and(|user| self.has_staff_role(&user.roles))
    }

    /// Whether the logged in user of a connection may use color codes and emoji.
    fn has_perks(&self, user_id: InternalId) -> bool {
        self.online_user(user_id).is_some_and(|user| {
            self.has_staff_role(&user.roles)
                || user.roles.iter().any(|role| self.config.message.perk_roles.contains(role))
        })
    }

    fn online_user(&self, user_id: InternalId) -> Option<&UserSession> {
        let stored_id = self.connections.get(&user_id)?.stored_id?;
        self.users.get(&stored_id)
    }

    /// Names are not unique: an account and Minecraft players may share one.
    fn find_by_name<'a>(&'a self, name: &'a str) -> impl Iterator<Item = (Uuid, &'a UserSession)> + 'a {
        self.users
            .iter()
            .filter(move |(_, user)| {
                user.connections
                    .iter()
                    .find_map(|id| self.connections.get(id)?.user.as_ref())
                    .is_some_and(|info| info.name == name)
            })
            .map(|(id, user)| (*id, user))
    }

    fn author(&self, user_id: InternalId) -> Option<Author> {
        let session = self.connections.get(&user_id)?;
        let (info, id) = (session.user.as_ref()?, session.stored_id?);
        let online = self.users.get(&id)?;
        let mut roles: Vec<RoleView> = online
            .roles
            .iter()
            .map(|id| match self.role(id) {
                Some(role) => RoleView {
                    id: role.id.clone(),
                    name: role.display_name.clone(),
                    staff: role.is_staff,
                },
                None => RoleView {
                    id: id.clone(),
                    name: id.clone(),
                    staff: false,
                },
            })
            .collect();
        let position = |role: &RoleView| self.roles.iter().position(|known| known.id == role.id).unwrap_or(usize::MAX);
        roles.sort_by_key(|role| (!role.staff, position(role)));
        Some(Author {
            id,
            kind: online.kind,
            name: info.name.clone(),
            uuid: info.uuid,
            minecraft: session.minecraft.clone().filter(|_| online.kind == Kind::Account),
            roles,
            highlight: self.has_perks(user_id),
        })
    }
}

impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) {
        info!("User `{}` disconnected.", msg.id);
        self.logout(msg.id);
        self.connections.remove(&msg.id);
    }
}

impl Handler<Malformed> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Malformed, _ctx: &mut Context<Self>) {
        warn!("Could not decode packet of `{}`: {}", msg.id, msg.error);
        // old clients never got an answer to these
        if self.connections.get(&msg.id).is_some_and(|session| session.hello) {
            self.send_error(msg.id, ClientError::InvalidPacket(msg.error));
        }
    }
}

pub(self) struct SessionState {
    addr: Recipient<ClientPacket>,
    ip: IpAddr,
    session_hash: Option<String>,
    /// Sent `Hello`, so it understands the packets added since the original protocol.
    hello: bool,
    user: Option<User>,
    /// The Minecraft account a Mojang or JWT login, or a proof after an account login, verified.
    minecraft: Option<UserInfo>,
    /// The id of the logged in user in the database.
    stored_id: Option<Uuid>,
}

impl SessionState {
    pub fn is_logged_in(&self) -> bool {
        self.user.is_some()
    }
}

struct UserSession {
    rate_limiter: RateLimiter,
    connections: HashSet<InternalId>,
    kind: Kind,
    /// Service API role ids.
    roles: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    /// A LiquidBounce Account.
    Account,
    Mojang,
}

#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect {
    id: InternalId,
}

#[derive(Message)]
#[rtype(result = "()")]
struct Malformed {
    id: InternalId,
    error: String,
}

/// A clientbound packet
#[derive(Message, Serialize, Clone)]
#[rtype(result = "()")]
#[serde(tag = "m", content = "c")]
enum ClientPacket {
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
    },
    #[serde(rename = "Error")]
    ErrorDetail {
        message: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Hello {
        protocol: u32,
    },
    Welcome {
        user: Author,
        staff: bool,
    },
    ChatMessage {
        channel: Channel,
        id: u64,
        time: i64,
        author: Author,
        content: String,
    },
}

#[derive(Serialize, Clone)]
struct Author {
    /// The id in the database.
    id: Uuid,
    kind: Kind,
    name: String,
    /// The Minecraft account shown for the user, nil if unknown.
    uuid: Uuid,
    /// The Minecraft account an account session proved it plays on.
    #[serde(skip_serializing_if = "Option::is_none")]
    minecraft: Option<UserInfo>,
    roles: Vec<RoleView>,
    /// Staff and perk roles stand out.
    highlight: bool,
}

#[derive(Serialize, Clone)]
struct RoleView {
    id: String,
    name: String,
    staff: bool,
}

/// A serverbound packet
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
#[serde(tag = "m", content = "c")]
enum ServerPacket {
    RequestMojangInfo,
    LoginMojang(User),
    LoginJWT { token: String, allow_messages: bool },
    LoginAccount { token: String, allow_messages: bool },
    Hello { protocol: u32 },
    RequestJWT,
    Message { content: String },
    PrivateMessage { receiver: String, content: String },
    BanUser { user: Uuid },
    UnbanUser { user: Uuid },
    RequestUserCount,
    ChatMessage { channel: Target, content: String },
}

#[derive(Message)]
#[rtype(result = "()")]
struct ServerPacketId {
    user_id: InternalId,
    packet: ServerPacket,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    pub name: String,
    pub uuid: Uuid,
    /// Should this user allow private messages?
    pub allow_messages: bool,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
enum SuccessReason {
    Login,
    Ban,
    Unban,
    /// An account session proved the Minecraft account it plays on.
    Minecraft,
}

#[cfg(test)]
mod tests {
    use super::{ClientError, ClientPacket};

    #[test]
    fn errors_keep_their_form() {
        let error = ClientError::InvalidCharacter('§');
        let old = serde_json::to_string(&ClientPacket::Error { message: error.clone() }).unwrap();
        assert_eq!(old, r#"{"m":"Error","c":{"message":{"InvalidCharacter":"§"}}}"#);
        let detailed = ClientPacket::ErrorDetail {
            message: error.code(),
            detail: error.detail(),
        };
        let detailed = serde_json::to_string(&detailed).unwrap();
        assert_eq!(detailed, r#"{"m":"Error","c":{"message":"InvalidCharacter","detail":"§"}}"#);
    }
}
