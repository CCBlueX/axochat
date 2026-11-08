use crate::ip::TrustedProxy;
use clap::Args;
use jsonwebtoken::Algorithm;
use rustrict::Type;
use std::{net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

#[derive(Args, Clone)]
pub struct Config {
    #[command(flatten)]
    pub api: ApiConfig,

    #[command(flatten)]
    pub mojang: MojangConfig,

    #[command(flatten)]
    pub net: NetConfig,

    #[command(flatten)]
    pub message: MsgConfig,

    #[command(flatten)]
    pub moderation: ModConfig,

    #[command(flatten)]
    pub filter: FilterConfig,

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
pub struct NetConfig {
    /// The addresses served over plain HTTP.
    #[arg(long = "server-addr", env = "SERVER_ADDR", value_delimiter = ',', default_value = "0.0.0.0:80")]
    pub address: Vec<SocketAddr>,
    /// The addresses served over HTTPS, if a certificate is set. 7886 is where existing clients connect.
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

    /// Service API roles that may use color codes and emoji; staff always may.
    #[arg(long = "perk-roles", env = "PERK_ROLES", value_delimiter = ',', default_value = "premium")]
    pub perk_roles: Vec<String>,
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
pub struct ModConfig {
    /// How long the last address of a user is kept after they were last seen.
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
pub struct FilterConfig {
    /// What checks global messages of users who are not staff: `none`, or any of `rustrict`
    /// (censors before delivery), `moderation` and `llm` (remove after delivery).
    #[arg(long = "message-filters", env = "MESSAGE_FILTERS", value_delimiter = ',', default_value = "none")]
    pub filters: Vec<Filter>,

    /// `type:severity` pairs rustrict censors, types `profane`, `offensive`, `sexual`, `mean`,
    /// severities `mild`, `moderate`, `severe` (that or worse).
    #[arg(long = "rustrict-censor", env = "RUSTRICT_CENSOR", value_delimiter = ',', default_value = "offensive:severe")]
    pub censor: Vec<CensorLevel>,

    /// The key for the OpenAI moderation endpoint.
    #[arg(id = "openai_api_key", long = "openai-api-key", env = "OPENAI_API_KEY", hide_env_values = true)]
    pub moderation_key: Option<String>,

    #[arg(long = "openai-url", env = "OPENAI_URL", default_value = "https://api.openai.com/v1", hide = true)]
    pub moderation_url: String,

    #[arg(long = "moderation-model", env = "MODERATION_MODEL", default_value = "omni-moderation-latest")]
    pub moderation_model: String,

    /// `category=score` pairs; a message scoring at least that in a listed category is removed.
    #[arg(
        long = "moderation-thresholds",
        env = "MODERATION_THRESHOLDS",
        value_delimiter = ',',
        default_value = "hate=0.88,hate/threatening=0.5"
    )]
    pub thresholds: Vec<Threshold>,

    /// An OpenAI-compatible Chat Completions API.
    #[arg(long = "llm-url", env = "LLM_URL", default_value = "https://api.openai.com/v1")]
    pub llm_url: String,

    #[arg(long = "llm-api-key", env = "LLM_API_KEY", hide_env_values = true)]
    pub llm_key: Option<String>,

    #[arg(long = "llm-model", env = "LLM_MODEL")]
    pub llm_model: Option<String>,

    /// A file with the system prompt, which asks for `REMOVE` or `KEEP`; a built-in one without.
    #[arg(long = "llm-prompt", env = "LLM_PROMPT")]
    pub llm_prompt: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum Filter {
    None,
    Rustrict,
    Moderation,
    Llm,
}

#[derive(Clone, Copy, Debug)]
pub struct CensorLevel(pub Type);

impl FromStr for CensorLevel {
    type Err = String;

    fn from_str(pair: &str) -> Result<CensorLevel, String> {
        let (kind, severity) = pair.split_once(':').ok_or("expected `type:severity`")?;
        let kind = match kind.trim() {
            "profane" => Type::PROFANE,
            "offensive" => Type::OFFENSIVE,
            "sexual" => Type::SEXUAL,
            "mean" => Type::MEAN,
            other => return Err(format!("unknown type `{}`", other)),
        };
        let severity = match severity.trim() {
            "mild" => Type::MILD_OR_HIGHER,
            "moderate" => Type::MODERATE_OR_HIGHER,
            "severe" => Type::SEVERE,
            other => return Err(format!("unknown severity `{}`", other)),
        };
        Ok(CensorLevel(kind & severity))
    }
}

#[derive(Clone, Debug)]
pub struct Threshold {
    pub category: String,
    pub score: f64,
}

impl FromStr for Threshold {
    type Err = String;

    fn from_str(pair: &str) -> Result<Threshold, String> {
        let (category, score) = pair.split_once('=').ok_or("expected `category=score`")?;
        let score: f64 = score.trim().parse().map_err(|_| format!("invalid score `{}`", score))?;
        if !(score > 0.0 && score <= 1.0) {
            return Err(format!("score `{}` is not in (0, 1]", score));
        }
        Ok(Threshold {
            category: category.trim().to_owned(),
            score,
        })
    }
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
    /// The LiquidBounce Service API, which checks account logins and knows their roles.
    #[arg(id = "api_url", long = "api-url", env = "API_URL", default_value = "https://api.liquidbounce.net")]
    pub url: String,

    /// The Service API token for roles and linked Minecraft accounts.
    #[arg(long = "api-token", env = "API_TOKEN", hide_env_values = true)]
    pub token: Option<String>,
}
