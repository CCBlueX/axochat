use crate::config::ApiConfig;

use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use uuid::Uuid;

pub struct Api {
    client: reqwest::Client,
    url: String,
    /// Where `/internal` is reachable, and the token for it.
    directory: Option<(String, String)>,
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Unavailable(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiError::Unauthorized => f.write_str("unauthorized"),
            ApiError::Unavailable(reason) => write!(f, "unavailable: {}", reason),
        }
    }
}

const MODERATE: &str = "moderate";

/// An account as `/me` and `/internal/users` answer it.
#[derive(Debug, Deserialize)]
pub struct Account {
    pub uid: String,
    /// Absent for an account that never signed in to the Service API.
    pub nickname: Option<String>,
    #[serde(default)]
    roles: Vec<RoleDefinition>,
    #[serde(default)]
    permissions: Vec<String>,
    minecraft: Option<MinecraftLink>,
}

impl Account {
    pub fn is_staff(&self) -> bool {
        self.permissions.iter().any(|permission| permission == MODERATE)
    }

    pub fn role_ids(&self) -> Vec<String> {
        self.roles.iter().map(|role| role.id.clone()).collect()
    }

    pub fn minecraft_uuid(&self) -> Option<Uuid> {
        self.minecraft.as_ref().map(|link| link.uuid)
    }
}

#[derive(Debug, Deserialize)]
struct MinecraftLink {
    uuid: Uuid,
}

#[derive(Deserialize)]
struct ChatClient {
    key: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDefinition {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    permissions: Vec<String>,
}

impl RoleDefinition {
    pub fn is_staff(&self) -> bool {
        self.permissions.iter().any(|permission| permission == MODERATE)
    }
}

#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
}

impl Api {
    pub fn new(config: &ApiConfig) -> Api {
        Api {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("could not build http client"),
            url: config.url.trim_end_matches('/').to_owned(),
            directory: config
                .internal_url
                .as_deref()
                .zip(config.directory_token.clone())
                .map(|(url, token)| (url.trim_end_matches('/').to_owned(), token)),
        }
    }

    /// The account an access token belongs to.
    pub async fn account(&self, access_token: &str) -> Result<Account, ApiError> {
        let request = self.client.get(format!("{}/me", self.url)).bearer_auth(access_token);
        fetch(request).await?.ok_or(ApiError::Unauthorized)
    }

    /// The account whose main Minecraft account is `uuid`.
    pub async fn linked_account(&self, uuid: Uuid) -> Result<Option<Account>, ApiError> {
        let Some(request) = self.internal(&format!("users?minecraftUuid={}", uuid)) else { return Ok(None) };
        let page: Option<Page<Account>> = fetch(request).await?;
        Ok(page.and_then(|page| page.items.into_iter().next()))
    }

    pub async fn roles(&self) -> Result<Vec<RoleDefinition>, ApiError> {
        let Some(request) = self.internal("roles") else { return Ok(Vec::new()) };
        Ok(fetch(request).await?.unwrap_or_default())
    }

    /// The name of every registered client by its key.
    pub async fn chat_clients(&self) -> Result<HashMap<String, String>, ApiError> {
        let Some(request) = self.internal("chat/clients") else { return Ok(HashMap::new()) };
        let clients: Vec<ChatClient> = fetch(request).await?.unwrap_or_default();
        Ok(clients.into_iter().map(|client| (client.key, client.name)).collect())
    }

    /// `None` for an account the Service API keeps nothing about.
    pub async fn account_by_uid(&self, uid: &str) -> Result<Option<Account>, ApiError> {
        // ids are plain, so they go into the URL as they are
        if uid.is_empty() || !uid.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return Ok(None);
        }
        let Some(request) = self.internal(&format!("users/{}", uid)) else { return Ok(None) };
        fetch(request).await
    }

    /// `None` without the settings for `/internal`.
    fn internal(&self, path: &str) -> Option<RequestBuilder> {
        let (url, token) = self.directory.as_ref()?;
        Some(self.client.get(format!("{}/internal/{}", url, path)).bearer_auth(token))
    }
}

/// `None` for 404.
async fn fetch<T: DeserializeOwned>(request: RequestBuilder) -> Result<Option<T>, ApiError> {
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Unavailable(err.to_string()))?;
    match response.status() {
        StatusCode::NOT_FOUND => Ok(None),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ApiError::Unauthorized),
        status if status.is_success() => response
            .json()
            .await
            .map(Some)
            .map_err(|err| ApiError::Unavailable(err.to_string())),
        status => Err(ApiError::Unavailable(status.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_comes_from_permissions() {
        let account: Account = serde_json::from_str(
            r#"{"uid":"a","roles":[{"id":"staff","displayName":"Staff","permissions":["moderate"]}],
                "permissions":["cosmetics","moderate"],"cosmetics":[],
                "minecraft":{"uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","linkedAt":"2026-01-01T00:00:00Z"}}"#,
        )
        .unwrap();
        assert!(account.is_staff());
        assert_eq!(account.role_ids(), ["staff"]);
        assert!(account.minecraft_uuid().is_some());

        let account: Account = serde_json::from_str(r#"{"uid":"a","roles":[{"id":"staff","displayName":"Staff","permissions":[]}],"permissions":[]}"#).unwrap();
        assert!(!account.is_staff());
    }
}
