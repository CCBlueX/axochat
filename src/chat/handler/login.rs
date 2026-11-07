use crate::api::Api;
use crate::auth::UserInfo;
use crate::chat::{new_id, now_ms, ChatServer, ClientPacket, InternalId, Kind, SuccessReason, User, UserSession};
use crate::entity::{punishment, user};
use crate::error::ClientError;
use crate::message::RateLimiter;
use crate::store::{Identify, IdentityKey, Store};
use log::*;

use actix::*;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;
use uuid::Uuid;

const MAX_NAME: usize = 32;

pub(super) struct Verified {
    pub model: user::Model,
    pub roles: Vec<String>,
    pub minecraft: Option<UserInfo>,
}

/// Taken before the login goes async.
pub(super) struct Visit {
    ip: IpAddr,
    id: Uuid,
    at: i64,
}

impl Visit {
    pub fn identify(self, key: IdentityKey, name: String, linked: Option<Uuid>) -> Identify {
        let name = match &key {
            IdentityKey::Account(account) => account_name(&name, account),
            IdentityKey::Minecraft(_) => name,
        };
        Identify {
            key,
            name: name.chars().take(64).collect(),
            linked,
            ip: self.ip,
            id: self.id,
            at: self.at,
        }
    }
}

impl ChatServer {
    pub(super) fn visit(&mut self, user_id: InternalId) -> Option<Visit> {
        let session = self.connections.get(&user_id)?;
        if session.is_logged_in() {
            info!("User `{}` tried to log in multiple times.", user_id);
            self.send_error(user_id, ClientError::AlreadyLoggedIn);
            return None;
        }
        Some(Visit {
            ip: session.ip,
            id: new_id(&mut self.rng),
            at: now_ms(),
        })
    }

    /// Lets a verified user in unless they are banned.
    pub(super) fn finish_login(&mut self, user_id: InternalId, result: Result<Verified, ClientError>, allow_messages: bool) {
        let Some(session) = self.connections.get(&user_id) else { return };
        if session.is_logged_in() {
            self.send_error(user_id, ClientError::AlreadyLoggedIn);
            return;
        }
        let verified = match result {
            Ok(verified) => verified,
            Err(error) => {
                self.send_error(user_id, error);
                return;
            }
        };

        let model = &verified.model;
        let banned = !self.has_staff_role(&verified.roles)
            && self
                .moderation
                .find(punishment::Kind::Ban, Some(model.id), Some(session.ip), now_ms())
                .is_some();
        if banned {
            info!("Banned user `{}` ({}) tried to log in.", model.name, model.id);
            self.send_error(user_id, ClientError::Banned);
            return;
        }

        let info = User {
            name: model.name.clone(),
            uuid: model.minecraft_uuid.or(model.linked_minecraft_uuid).unwrap_or_else(Uuid::nil),
            allow_messages,
        };
        info!("User `{}` logged in as `{}` ({}).", user_id, info.name, model.id);
        let kind = if model.account.is_some() { Kind::Account } else { Kind::Mojang };
        let message_config = &self.config.message;
        let online = self.users.entry(model.id).or_insert_with(|| UserSession {
            rate_limiter: RateLimiter::new(message_config.clone()),
            connections: HashSet::new(),
            kind,
            roles: Vec::new(),
        });
        online.connections.insert(user_id);
        online.roles = verified.roles;

        let session = self.connections.get_mut(&user_id).expect("checked above");
        session.user = Some(info);
        session.stored_id = Some(verified.model.id);
        session.minecraft = verified.minecraft;
        session.session_hash = None;

        if session.hello {
            if let Some(user) = self.author(user_id) {
                let staff = self.is_staff(user_id);
                self.send(user_id, ClientPacket::Welcome { user, staff });
            }
        }
        self.send(user_id, ClientPacket::Success { reason: SuccessReason::Login });
    }
}

/// A Minecraft account linked to a LiquidBounce Account logs in as that account.
pub(super) async fn login_minecraft(
    api: Arc<Api>,
    store: Addr<Store>,
    visit: Visit,
    player: UserInfo,
) -> Result<Verified, ClientError> {
    let linked = api.linked_account(player.uuid).await.unwrap_or_else(|err| {
        warn!("Could not look up the account linked to `{}`: {}", player.uuid, err);
        None
    });
    let (request, roles) = match linked {
        Some(account) => {
            let name = account.nickname.unwrap_or_else(|| player.name.clone());
            (visit.identify(IdentityKey::Account(account.user_id), name, Some(player.uuid)), account.roles)
        }
        None => (visit.identify(IdentityKey::Minecraft(player.uuid), player.name.clone(), None), Vec::new()),
    };
    let model = identify(store, request).await?;
    Ok(Verified { model, roles, minecraft: Some(player) })
}

pub(super) async fn identify(store: Addr<Store>, request: Identify) -> Result<user::Model, ClientError> {
    match store.send(request).await {
        Ok(Ok(model)) => Ok(model),
        Ok(Err(err)) => {
            error!("Could not store user: {}", err);
            Err(ClientError::Internal)
        }
        Err(err) => {
            error!("Store unavailable: {}", err);
            Err(ClientError::Internal)
        }
    }
}

/// Nicknames are free text: formatting codes, invisible and direction-changing characters go, and
/// whitespace becomes `_` so the name can be addressed.
fn account_name(nickname: &str, account: &str) -> String {
    let mut name = String::new();
    let mut gap = false;
    for ch in nickname.chars() {
        if ch.is_whitespace() {
            gap = !name.is_empty();
        } else if ch != '§' && (ch.is_ascii_graphic() || ch.is_alphanumeric()) {
            if gap {
                name.push('_');
                gap = false;
            }
            name.push(ch);
        }
    }
    let name: String = name.chars().take(MAX_NAME).collect();
    if name.is_empty() {
        format!("user-{}", account.chars().take(8).collect::<String>())
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::account_name;

    #[test]
    fn account_names() {
        assert_eq!(account_name("  Izuna  Seikatsu ", "a"), "Izuna_Seikatsu");
        assert_eq!(account_name("§cStaff", "a"), "cStaff");
        assert_eq!(account_name("a\u{202e}b\u{200b}c\nd", "a"), "abc_d");
        assert_eq!(account_name("ünïcödé", "a"), "ünïcödé");
        assert_eq!(account_name("\u{200b}", "0123456789"), "user-01234567");
        assert_eq!(account_name(&"x".repeat(40), "a").len(), 32);
    }
}
