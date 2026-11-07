mod connect;
mod handler;
mod id;
mod session;

pub use id::*;

use crate::config::Config;
use crate::error::*;
use log::*;

use actix::*;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::auth::{Authenticator, UserInfo};
use crate::ip::RealIp;
use crate::message::{MessageValidator, RateLimiter};
use crate::moderation::Moderation;
use crate::store::{Persist, Store, Write};
use rand::{rngs::SysRng, SeedableRng};
use rand_hc::Hc128Rng;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Duration;
use uuid::Uuid;

const MAINTENANCE: Duration = Duration::from_secs(3600);

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
    users: HashMap<String, UserSession>,

    rng: rand_hc::Hc128Rng,
    authenticator: Option<Authenticator>,
    validator: MessageValidator,
    moderation: Moderation,
    store: Addr<Store>,
    config: Config,

    current_internal_user_id: u64,
}

impl ChatServer {
    pub fn new(config: Config, store: Addr<Store>, moderation: Moderation) -> ChatServer {
        ChatServer {
            connections: HashMap::new(),
            users: HashMap::new(),

            rng: Hc128Rng::try_from_rng(&mut SysRng).expect("could not initialize hc128 rng"),
            authenticator: Authenticator::new(&config.auth),
            validator: MessageValidator::new(config.message.clone()),
            moderation,
            store,
            config,

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

    /// Ends the login of a connection, which stays open.
    fn logout(&mut self, id: InternalId) {
        let Some(info) = self.connections.get_mut(&id).and_then(|session| {
            session.stored_id = None;
            session.user.take()
        }) else {
            return;
        };
        if let Some(user_session) = self.users.get_mut(&info.name) {
            user_session.connections.remove(&id);
            if user_session.connections.is_empty() {
                self.users.remove(&info.name);
            }
        }
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

pub(self) struct SessionState {
    addr: Recipient<ClientPacket>,
    ip: IpAddr,
    session_hash: Option<String>,
    user: Option<User>,
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
}

#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect {
    id: InternalId,
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
}

/// A serverbound packet
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
#[serde(tag = "m", content = "c")]
enum ServerPacket {
    RequestMojangInfo,
    LoginMojang(User),
    LoginJWT { token: String, allow_messages: bool },
    RequestJWT,
    Message { content: String },
    PrivateMessage { receiver: String, content: String },
    BanUser { user: Uuid },
    UnbanUser { user: Uuid },
    RequestUserCount,
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
}
