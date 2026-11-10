use super::login::{account_name, identify};
use super::{ChatServer, ClientPacket};
use crate::api::{Api, ApiError};
use crate::auth::UserInfo;
use crate::chat::history::Audience;
use crate::chat::{
    new_id, now_ms, send_message, Author, InternalId, Kind as UserKind, Relayed, Target, ROLE_REFRESH,
};
use crate::entity::{punishment::Kind, user};
use crate::error::*;
use crate::message::RateLimiter;
use crate::moderation::Punishment;
use crate::store::{Identify, IdentityKey, Store};
use log::*;

use actix::*;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// A logged in user allowed to send a message.
struct Sender {
    /// The id in the database.
    id: Uuid,
    legacy: UserInfo,
    author: Author,
    /// Staff messages are not screened.
    screened: bool,
}

impl ChatServer {
    pub(super) fn handle_message(&mut self, user_id: InternalId, content: String, ctx: &mut Context<Self>) {
        let Some(sender) = self.check_message(user_id, &content) else { return };
        self.send_global(user_id, sender, content, ctx);
    }

    /// Old clients reach Minecraft accounts too, by name only.
    pub(super) fn handle_private_message(&mut self, user_id: InternalId, receiver: String, content: String) {
        let Some(sender) = self.check_message(user_id, &content) else { return };
        let target = self
            .find_by_name(&receiver)
            .min_by_key(|(_, user)| user.kind == UserKind::Account)
            .map(|(id, _)| id);
        self.send_direct(user_id, sender, target, content);
    }

    pub(super) fn handle_chat_message(
        &mut self,
        user_id: InternalId,
        channel: Target,
        content: String,
        ctx: &mut Context<Self>,
    ) {
        match channel {
            Target::Global => self.handle_message(user_id, content, ctx),
            Target::Direct(receiver) => {
                if self.online_user(user_id).is_some_and(|user| user.kind != UserKind::Account) {
                    self.send_error(user_id, ClientError::AccountRequired);
                    return;
                }
                let Some(sender) = self.check_message(user_id, &content) else { return };
                let target = self.find_account(&receiver);
                self.send_direct(user_id, sender, target, content);
            }
            Target::Unknown(channel) => self.send_error(user_id, ClientError::UnknownChannel(channel)),
        }
    }

    /// Online users by id or name. Minecraft accounts are left out: nobody may tell whether one uses the chat.
    fn find_account(&self, user: &str) -> Option<Uuid> {
        let by_id = Uuid::parse_str(user)
            .ok()
            .filter(|id| self.users.get(id).is_some_and(|user| user.kind == UserKind::Account));
        by_id.or_else(|| {
            self.find_by_name(user)
                .find(|(_, user)| user.kind == UserKind::Account)
                .map(|(id, _)| id)
        })
    }

    fn send_global(&mut self, user_id: InternalId, sender: Sender, content: String, ctx: &mut Context<Self>) {
        info!("User `{}` has written `{}`.", user_id, content);
        let (content, original) = match self.filters.censor(&content).filter(|_| sender.screened) {
            Some(censored) => (censored, Some(content)),
            None => (content, None),
        };
        let legacy = ClientPacket::Message {
            author_info: sender.legacy,
            content: content.clone(),
        };
        let id = self.history.record(sender.id, Audience::Global, content, original).id;
        let message = self.history.get(id).expect("just recorded");
        for (id, session) in &self.connections {
            let packet = if session.hello {
                message.packet(session.stored_id, &sender.author, self.is_staff(*id))
            } else {
                legacy.clone()
            };
            send_message(&session.addr, packet, "broadcast message");
        }
        if sender.screened && !self.filters.screens.is_empty() {
            let (id, content) = (message.id, message.content.clone());
            self.screen(id, content, ctx);
        }
    }

    /// Every screen looks at what was delivered on its own; the first to object takes the message back.
    fn screen(&mut self, id: u64, content: String, ctx: &mut Context<Self>) {
        let content = Arc::new(content);
        for screen in self.filters.screens.clone() {
            let content = content.clone();
            let check = async move { (screen.name(), screen.violation(&content).await) };
            ctx.spawn(check.into_actor(self).map(move |(name, result), actor, _ctx| match result {
                Ok(Some(reason)) => {
                    let Some(author) = actor.history.get(id).filter(|recorded| !recorded.removed).map(|r| r.author)
                    else {
                        return;
                    };
                    info!("The {} filter removed message {} for `{}`.", name, id, reason);
                    let muted = actor.filter_mute(author, &reason);
                    actor.remove_message(id, Some(reason), muted);
                }
                Ok(None) => {}
                Err(err) => warn!("The {} filter could not check message {}: {}", name, id, err),
            }));
        }
    }

    /// Mutes the author of a removed message unless they already are; when the mute ends.
    fn filter_mute(&mut self, author: Uuid, reason: &str) -> Option<i64> {
        let duration = self.config.filter.mute?;
        let now = now_ms();
        if self.moderation.find(Kind::Mute, Some(author), None, now).is_some() {
            return None;
        }
        let expires_at = now.saturating_add(duration.as_millis() as i64);
        let id = new_id(&mut self.rng);
        self.punish(Punishment {
            id,
            kind: Kind::Mute,
            user: Some(author),
            ip: None,
            reason: format!("Removed message: {}", reason),
            issued_by: None,
            created_at: now,
            expires_at: Some(expires_at),
        });
        Some(expires_at)
    }

    pub(super) fn handle_remove_message(&mut self, user_id: InternalId, id: u64) {
        if !self.is_staff(user_id) {
            self.send_error(user_id, ClientError::NotPermitted);
            return;
        }
        if self.history.get(id).is_none() {
            self.send_error(user_id, ClientError::InvalidId);
        } else if self.remove_message(id, None, None) {
            info!("`{}` removed message {}.", user_id, id);
        }
    }

    /// Tells everyone who could have seen the message, staff also why and how long the author is muted;
    /// clients without a handshake cannot be told. False if it was already removed or is too old.
    fn remove_message(&mut self, id: u64, reason: Option<String>, muted: Option<i64>) -> bool {
        let Some(recorded) = self.history.get_mut(id).filter(|recorded| !recorded.removed) else { return false };
        recorded.removed = true;
        let (author, audience) = (recorded.author, recorded.audience);
        let viewers = self.connections.iter().filter(|(_, session)| {
            session.hello
                && match audience {
                    Audience::Global => true,
                    Audience::Direct { receiver } => session.stored_id.is_some_and(|user| user == author || user == receiver),
                }
        });
        for (viewer, session) in viewers {
            let packet = if self.is_staff(*viewer) {
                ClientPacket::MessageRemoved { id, reason: reason.clone(), muted }
            } else {
                ClientPacket::MessageRemoved { id, reason: None, muted: None }
            };
            send_message(&session.addr, packet, "removal");
        }
        true
    }

    /// Reaches every connection of the receiver that allows messages; the sender's other connections see it too.
    fn send_direct(&mut self, user_id: InternalId, sender: Sender, target: Option<Uuid>, content: String) {
        let receivers: Vec<InternalId> = target
            .and_then(|target| self.users.get(&target))
            .into_iter()
            .flat_map(|user| user.connections.iter().copied())
            .filter(|id| {
                self.connections.get(id).is_some_and(|session| {
                    session.addr.connected() && session.user.as_ref().is_some_and(|user| user.allow_messages)
                })
            })
            .collect();
        let Some(target) = target.filter(|_| !receivers.is_empty()) else {
            self.send_error(user_id, ClientError::PrivateMessageNotAccepted);
            return;
        };

        info!("User `{}` has written to `{}` privately.", user_id, target);
        let legacy = ClientPacket::PrivateMessage {
            author_info: sender.legacy,
            content: content.clone(),
        };
        let message = self.history.record(sender.id, Audience::Direct { receiver: target }, content, None);
        let echoes = self
            .users
            .get(&sender.id)
            .into_iter()
            .flat_map(|user| user.connections.iter())
            .filter(|id| !receivers.contains(id))
            .filter_map(|id| self.connections.get(id).filter(|session| session.hello));
        let receivers = receivers.iter().filter_map(|id| self.connections.get(id));
        for session in receivers.chain(echoes) {
            let packet = if session.hello {
                message.packet(session.stored_id, &sender.author, false)
            } else {
                legacy.clone()
            };
            send_message(&session.addr, packet, "private message");
        }
    }

    fn check_message(&mut self, user_id: InternalId, content: &str) -> Option<Sender> {
        if self.check_ratelimit(user_id, content.to_owned()) {
            return None;
        }

        let session = self.connections.get(&user_id)?;
        let (Some(info), Some(id)) = (&session.user, session.stored_id) else {
            info!("`{}` is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return None;
        };

        if let Err(err) = self.validator.validate(content, self.has_perks(user_id)) {
            info!("User `{}` tried to send invalid message: {}", user_id, err);
            if let Error::AxoChat { source } = err {
                self.send_error(user_id, source);
            }
            return None;
        }
        let staff = self.is_staff(user_id);
        let muted = !staff
            && self
                .moderation
                .find(Kind::Mute, Some(id), Some(session.ip), now_ms())
                .is_some();
        if muted {
            info!("User `{}` tried to send message while muted", user_id);
            let error = if session.hello { ClientError::Muted } else { ClientError::Banned };
            self.send_error(user_id, error);
            return None;
        }

        Some(Sender {
            id,
            legacy: UserInfo {
                name: info.name.clone(),
                uuid: info.uuid,
            },
            author: self.author(user_id)?,
            screened: !staff,
        })
    }

    /// A service session sending a message for a LiquidBounce Account, which then counts as that account's.
    pub(super) fn handle_message_for(
        &mut self,
        user_id: InternalId,
        channel: Target,
        content: String,
        account: String,
        ctx: &mut Context<Self>,
    ) {
        let Some(user) = self.online_user(user_id) else {
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        };
        if !user.service {
            self.send_error_for(user_id, ClientError::NotPermitted, &account);
            return;
        }
        if let Target::Direct(_) | Target::Unknown(_) = channel {
            self.send_error_for(user_id, ClientError::UnknownChannel(account.clone()), &account);
            return;
        }

        let fresh = self
            .relayed
            .iter()
            .find(|(_, relayed)| relayed.account == account && relayed.fetched_at.elapsed() < ROLE_REFRESH)
            .map(|(id, _)| *id);
        if let Some(id) = fresh {
            self.send_for(user_id, id, &account, content, ctx);
            return;
        }

        let lookup = account_user(self.api.clone(), self.store.clone(), account.clone(), new_id(&mut self.rng), now_ms());
        ctx.spawn(lookup.into_actor(self).map(move |result, actor, ctx| match result {
            Ok((model, roles)) => {
                let legacy = UserInfo {
                    name: model.name.clone(),
                    uuid: model.minecraft_uuid.or(model.linked_minecraft_uuid).unwrap_or_else(Uuid::nil),
                };
                let message_config = &actor.config.message;
                let relayed = actor.relayed.entry(model.id).or_insert_with(|| Relayed {
                    account: account.clone(),
                    legacy: legacy.clone(),
                    roles: Vec::new(),
                    rate_limiter: RateLimiter::new(message_config.clone()),
                    fetched_at: Instant::now(),
                });
                relayed.legacy = legacy;
                relayed.roles = roles;
                relayed.fetched_at = Instant::now();
                actor.send_for(user_id, model.id, &account, content, ctx);
            }
            Err(error) => actor.send_error_for(user_id, error, &account),
        }));
    }

    fn send_for(&mut self, user_id: InternalId, id: Uuid, account: &str, content: String, ctx: &mut Context<Self>) {
        match self.check_message_for(id, &content) {
            Ok(sender) => self.send_global(user_id, sender, content, ctx),
            Err(error) => {
                info!("Message of `{}` for `{}` was refused: {}", user_id, account, error);
                self.send_error_for(user_id, error, account);
            }
        }
    }

    /// The checks of [ChatServer::check_message] for an account a service session sends for.
    fn check_message_for(&mut self, id: Uuid, content: &str) -> std::result::Result<Sender, ClientError> {
        let relayed = self.relayed.get(&id).ok_or(ClientError::Internal)?;
        let roles = relayed.roles.clone();
        let legacy = relayed.legacy.clone();
        let (staff, perks) = (self.has_staff_role(&roles), self.has_perk_role(&roles));
        let now = now_ms();
        if !staff && self.moderation.find(Kind::Ban, Some(id), None, now).is_some() {
            return Err(ClientError::Banned);
        }

        let rate_limiter = match self.users.get_mut(&id) {
            Some(online) => &mut online.rate_limiter,
            None => &mut self.relayed.get_mut(&id).ok_or(ClientError::Internal)?.rate_limiter,
        };
        if rate_limiter.check_new_message(content.to_owned()) {
            return Err(ClientError::RateLimited);
        }
        self.validator.validate(content, perks).map_err(|err| match err {
            Error::AxoChat { source } => source,
            _ => ClientError::Internal,
        })?;
        if !staff && self.moderation.find(Kind::Mute, Some(id), None, now).is_some() {
            return Err(ClientError::Muted);
        }

        Ok(Sender {
            id,
            author: Author {
                id,
                kind: UserKind::Account,
                name: legacy.name.clone(),
                uuid: legacy.uuid,
                minecraft: None,
                roles: self.role_views(&roles),
                highlight: perks,
            },
            legacy,
            screened: !staff,
        })
    }

    fn check_ratelimit(&mut self, user_id: InternalId, message: String) -> bool {
        let Some(stored_id) = self.connections.get(&user_id).and_then(|session| session.stored_id) else {
            return false;
        };
        let Some(user) = self.users.get_mut(&stored_id) else { return false };
        if user.rate_limiter.check_new_message(message) {
            info!("User `{}` tried to send message, but was rate limited.", user_id);
            self.send_error(user_id, ClientError::RateLimited);
            true
        } else {
            false
        }
    }
}

/// The user of a LiquidBounce Account, created like on its first login, and its roles.
async fn account_user(
    api: Arc<Api>,
    store: Addr<Store>,
    account: String,
    id: Uuid,
    at: i64,
) -> std::result::Result<(user::Model, Vec<String>), ClientError> {
    let unavailable = |err: ApiError| {
        warn!("Could not look up account `{}`: {}", account, err);
        ClientError::Internal
    };
    let profile = api
        .profile(&account)
        .await
        .map_err(unavailable)?
        .ok_or_else(|| ClientError::UnknownUser(account.clone()))?;
    let roles = api.account_roles(&account).await.map_err(unavailable)?;
    let request = Identify {
        key: IdentityKey::Account(account.clone()),
        name: account_name(&profile.nickname, &account),
        linked: None,
        ip: None,
        id,
        at,
    };
    Ok((identify(store, request).await?, roles))
}
