use super::{ChatServer, ClientPacket};
use crate::auth::UserInfo;
use crate::chat::history::Audience;
use crate::chat::{now_ms, send_message, Author, InternalId, Kind as UserKind, Target};
use crate::entity::punishment::Kind;

use crate::error::*;
use log::*;
use uuid::Uuid;

/// A logged in user allowed to send a message.
struct Sender {
    /// The id in the database.
    id: Uuid,
    legacy: UserInfo,
    author: Author,
}

impl ChatServer {
    pub(super) fn handle_message(&mut self, user_id: InternalId, content: String) {
        let Some(sender) = self.check_message(user_id, &content) else { return };
        self.send_global(user_id, sender, content);
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

    pub(super) fn handle_chat_message(&mut self, user_id: InternalId, channel: Target, content: String) {
        match channel {
            Target::Global => self.handle_message(user_id, content),
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

    fn send_global(&mut self, user_id: InternalId, sender: Sender, content: String) {
        info!("User `{}` has written `{}`.", user_id, content);
        let legacy = ClientPacket::Message {
            author_info: sender.legacy,
            content: content.clone(),
        };
        let message = self.history.record(sender.id, Audience::Global, content);
        for session in self.connections.values() {
            let packet = if session.hello {
                message.packet(session.stored_id, &sender.author)
            } else {
                legacy.clone()
            };
            send_message(&session.addr, packet, "broadcast message");
        }
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
        let message = self.history.record(sender.id, Audience::Direct { receiver: target }, content);
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
                message.packet(session.stored_id, &sender.author)
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
        let muted = !self.is_staff(user_id)
            && self
                .moderation
                .find(Kind::Mute, Some(id), Some(session.ip), now_ms())
                .is_some();
        if muted {
            info!("User `{}` tried to send message while muted", user_id);
            self.send_error(user_id, ClientError::Banned);
            return None;
        }

        Some(Sender {
            id,
            legacy: UserInfo {
                name: info.name.clone(),
                uuid: info.uuid,
            },
            author: self.author(user_id)?,
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
