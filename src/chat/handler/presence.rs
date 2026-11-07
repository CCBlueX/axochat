use crate::chat::presence::Presence;
use crate::chat::{ChatServer, ClientPacket, InternalId};
use crate::error::*;
use log::*;

use actix::*;
use std::time::{Duration, Instant};

const PRESENCE_INTERVAL: Duration = Duration::from_secs(1);

impl ChatServer {
    pub(super) fn handle_presence(&mut self, user_id: InternalId, presence: Presence) {
        let Some(session) = self.connections.get(&user_id) else { return };
        if !session.is_logged_in() {
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        }
        let presence = match presence.validated() {
            Ok(presence) => presence,
            Err(reason) => return self.send_error(user_id, ClientError::InvalidPacket(reason)),
        };
        let Some(session) = self.connections.get_mut(&user_id) else { return };
        let now = Instant::now();
        if session.presence.as_ref().is_some_and(|(at, _)| now.duration_since(*at) < PRESENCE_INTERVAL) {
            return;
        }
        session.presence = Some((now, presence));
    }

    pub(super) fn handle_request_presence(&mut self, user_id: InternalId, user: String, ctx: &mut Context<Self>) {
        if self.moderator(user_id).is_none() {
            return;
        }
        self.find_user(ctx, user_id, user, move |actor, found| {
            let presence = actor
                .users
                .get(&found.user.id)
                .into_iter()
                .flat_map(|online| &online.connections)
                .filter_map(|id| actor.connections.get(id)?.presence.as_ref())
                .max_by_key(|(at, _)| *at)
                .map(|(_, presence)| presence.clone());
            info!("`{}` requested the presence of `{}`.", user_id, found.user.name);
            actor.send(user_id, ClientPacket::Presence { user: found.user, presence });
        });
    }
}
