use crate::ip::TrustedProxy;
use clap::Args;
use jsonwebtoken::Algorithm;
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Args, Clone)]
pub struct Config {
    #[command(flatten)]
    pub api: ApiConfig,

    #[command(flatten)]
    pub mojang: MojangConfig,

    #[command(flatten)]
    pub database: DbConfig,

    #[command(flatten)]
    pub net: NetConfig,

    #[command(flatten)]
    pub message: MsgConfig,

    #[command(flatten)]
    pub moderation: ModConfig,

    #[command(flatten)]
    pub service: ServiceConfig,

    #[command(flatten)]
    pub auth: AuthConfig,
}

#[derive(Args, Clone)]
pub struct ServiceConfig {
    /// A token that logs in as a staff account, for tooling. Unset disables it.
    #[arg(id = "service_token", long = "service-token", env = "SERVICE_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// The name of the account the service token logs in as.
    #[arg(id = "service_name", long = "service-name", env = "SERVICE_NAME", default_value = "LiquidChat")]
    pub name: String,
}

#[derive(Args, Clone)]
pub struct AuthConfig {
    /// The key of the JWT. JWT login is disabled without it.
    #[arg(
        long = "jwt-secret",
        env = "JWT_SECRET",
        hide_env_values = true,
        requires = "valid_time"
    )]
    pub secret: Option<String>,

    /// The JWT algorithm
    #[arg(long = "jwt-algorithm", env = "JWT_ALGORITHM", default_value = "HS256")]
    pub algorithm: Algorithm,

    /// The time for which a JWT is valid
    #[arg(long = "jwt-valid-time", env = "JWT_VALID_TIME", value_parser = humantime::parse_duration)]
    pub valid_time: Option<Duration>,
}

#[derive(Args, Clone)]
pub struct NetConfig {
    /// The addresses served over plain HTTP.
    #[arg(long = "server-addr", env = "SERVER_ADDR", value_delimiter = ',', default_value = "0.0.0.0:80")]
    pub address: Vec<SocketAddr>,
    /// The addresses served over HTTPS, if a certificate is set. 7886 is where v1 clients connect.
    #[arg(long = "tls-addr", env = "TLS_ADDR", value_delimiter = ',', default_value = "0.0.0.0:443,0.0.0.0:7886")]
    pub tls_address: Vec<SocketAddr>,

    /// The SSL certificate file.
    #[arg(long = "tls-cert-file", env = "TLS_CERT_FILE")]
    pub cert_file: Option<PathBuf>,
    /// The SSL key file.
    /// If the extension is `pem`, `PEM` format will be used, otherwise `ASN1`.
    #[arg(long = "tls-key-file", env = "TLS_KEY_FILE")]
    pub key_file: Option<PathBuf>,

    /// Proxies whose client address header is believed: CIDRs, or `cloudflare` for its ranges.
    #[arg(long = "trusted-proxies", env = "TRUSTED_PROXIES", value_delimiter = ',')]
    pub trusted_proxies: Vec<TrustedProxy>,

    /// The header a trusted proxy puts the client address in.
    #[arg(long = "real-ip-header", env = "REAL_IP_HEADER", default_value = "CF-Connecting-IP")]
    pub real_ip_header: String,
}

#[derive(Args, Clone)]
pub struct MsgConfig {
    /// The maximum message length in chars.
    #[arg(long = "message-max-length", env = "MESSAGE_MAX_LENGTH", default_value_t = 100)]
    pub max_length: usize,

    /// The maximum amount of messages in `count_duration`.
    #[arg(long = "message-max-messages", env = "MESSAGE_MAX_MESSAGES", default_value_t = 40)]
    pub max_messages: usize,

    /// The duration in which the amount of messages cannot be greater.
    #[arg(
        long = "message-count-duration",
        env = "MESSAGE_COUNT_DURATION",
        default_value = "1m",
        value_parser = humantime::parse_duration
    )]
    pub count_duration: Duration,

    /// Service API roles that may use color codes and emoji and get highlighted messages.
    #[arg(long = "perk-roles", env = "PERK_ROLES", value_delimiter = ',', default_value = "premium")]
    pub perk_roles: Vec<String>,
}

#[derive(Args, Clone)]
pub struct ModConfig {
    /// How long the last address of a user is kept for IP bans after they were last seen.
    #[arg(
        long = "ip-retention",
        env = "IP_RETENTION",
        default_value = "30d",
        value_parser = humantime::parse_duration
    )]
    pub ip_retention: Duration,
}

#[derive(Args, Clone)]
pub struct DbConfig {
    /// The MySQL or MariaDB connection URL.
    #[arg(id = "database_url", long = "database-url", env = "DATABASE_URL", hide_env_values = true)]
    pub url: String,
}

#[derive(Args, Clone)]
pub struct MojangConfig {
    #[arg(
        long = "mojang-session-url",
        env = "MOJANG_SESSION_URL",
        default_value = "https://sessionserver.mojang.com",
        hide = true
    )]
    pub session_url: String,
}

#[derive(Args, Clone)]
pub struct ApiConfig {
    /// The LiquidBounce Service API, which checks LiquidBounce Account logins.
    #[arg(id = "api_url", long = "api-url", env = "API_URL", default_value = "https://api.liquidbounce.net")]
    pub url: String,

    /// The Service API token for role names, staff and linked Minecraft accounts.
    #[arg(long = "api-token", env = "API_TOKEN", hide_env_values = true)]
    pub token: Option<String>,
}
