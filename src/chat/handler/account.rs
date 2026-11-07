use super::login::{identify, Verified};
use crate::api::ApiError;
use crate::chat::{ChatServer, InternalId};
use crate::error::ClientError;
use crate::store::IdentityKey;
use log::*;

use actix::*;

/// The account key of the user the service token logs in as.
const SERVICE_ACCOUNT: &str = "service";

impl ChatServer {
    pub(super) fn login_account(&mut self, user_id: InternalId, token: String, allow_messages: bool, ctx: &mut Context<Self>) {
        if self.is_service_token(&token) {
            self.login_service(user_id, allow_messages, ctx);
            return;
        }
        let Some(visit) = self.begin_login(user_id, true) else { return };
        let delay = visit.delay;
        let (api, store, logins) = (self.api.clone(), self.store.clone(), self.logins.clone());

        ctx.spawn(
            async move {
                actix::clock::sleep(delay).await;
                let _permit = logins.acquire().await;
                let account = api.account(&token).await.map_err(|err| match err {
                    ApiError::Unauthorized => ClientError::LoginFailed,
                    ApiError::Unavailable(reason) => {
                        warn!("Could not check account of `{}`: {}", user_id, reason);
                        ClientError::Internal
                    }
                })?;

                let request = visit.identify(IdentityKey::Account(account.user_id), account.nickname, account.minecraft_uuid);
                let model = identify(store, request).await?;
                Ok(Verified { model, roles: account.roles, minecraft: None, service: false })
            }
            .into_actor(self)
            .map(move |result, actor, _ctx| actor.finish_login(user_id, result, allow_messages)),
        );
    }

    fn is_service_token(&self, token: &str) -> bool {
        self.config
            .service
            .token
            .as_deref()
            .is_some_and(|expected| !expected.is_empty() && constant_time_eq(expected.as_bytes(), token.as_bytes()))
    }

    fn login_service(&mut self, user_id: InternalId, allow_messages: bool, ctx: &mut Context<Self>) {
        let Some(visit) = self.begin_login(user_id, false) else { return };
        let request = visit.identify(IdentityKey::Account(SERVICE_ACCOUNT.into()), self.config.service.name.clone(), None);
        let store = self.store.clone();
        ctx.spawn(
            async move {
                let model = identify(store, request).await?;
                Ok(Verified { model, roles: Vec::new(), minecraft: None, service: true })
            }
            .into_actor(self)
            .map(move |result, actor, _ctx| actor.finish_login(user_id, result, allow_messages)),
        );
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn compares_tokens() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret2"));
    }
}
