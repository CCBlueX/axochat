mod api;
mod auth;
mod chat;
mod config;
mod entity;
mod error;
mod ip;
mod message;
mod moderation;
mod filter;
mod store;

use clap::{Parser, Subcommand};
use config::{Config, DbConfig};
use error::*;
use log::*;

use actix::*;
use actix_web::{web, App, HttpServer};
use rand::{rngs::SysRng, SeedableRng};
use rand_hc::Hc128Rng;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[cfg(feature = "rustls-tls")]
use rustls::{
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    ServerConfig,
};

#[cfg(feature = "openssl-tls")]
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

#[derive(Parser)]
struct Opt {
    #[command(flatten)]
    config: Config,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Starts the axochat server.
    Start {
        #[command(flatten)]
        database: DbConfig,
    },
    /// Generates a JWT which can be used for logging in.
    /// This should only be used for testing.
    /// If you want to generate JWT for non-testing purposes, send a RequestJWT packet to the server.
    Generate {
        name: String,
        uuid: Option<Uuid>,
    },
    /// Mutes the Minecraft UUIDs of a line-separated file, like the old `banned.txt`.
    ImportBans {
        #[command(flatten)]
        database: DbConfig,
        file: PathBuf,
    },
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();

    let Opt { config, command } = Opt::parse();
    match command {
        Command::Start { database } => start_server(config, &database).await,
        Command::Generate { name, uuid } => {
            let auth = match auth::Authenticator::new(&config.auth) {
                Some(auth) => auth,
                None => {
                    eprintln!("Set JWT_SECRET to generate tokens.");
                    return Err(ClientError::NotSupported.into());
                }
            };
            let token = auth.new_token(auth::UserInfo {
                name,
                uuid: uuid.unwrap_or_else(|| Uuid::from_u128(0)),
            })?;
            println!("{}", token);
            Ok(())
        }
        Command::ImportBans { database, file } => import_bans(&database, &file).await,
    }
}

async fn open_store(database: &DbConfig) -> Result<Addr<store::Store>> {
    let db = sea_orm::Database::connect(&database.url).await?;
    db.get_schema_registry("axochat::entity::*").sync(&db).await?;
    Ok(store::Store::new(db).start())
}

async fn active_punishments(store: &Addr<store::Store>) -> Result<Vec<moderation::Punishment>> {
    let models = store.send(store::LoadPunishments { now: chat::now_ms() }).await??;
    Ok(models.into_iter().map(moderation::Punishment::from_model).collect())
}

async fn import_bans(database: &DbConfig, file: &Path) -> Result<()> {
    let store = open_store(database).await?;
    let mut muted: HashSet<_> = active_punishments(&store)
        .await?
        .into_iter()
        .filter(|punishment| punishment.kind == entity::punishment::Kind::Mute)
        .filter_map(|punishment| punishment.user)
        .collect();
    let mut rng = Hc128Rng::try_from_rng(&mut SysRng).expect("could not initialize hc128 rng");

    let mut imported = 0;
    for line in std::fs::read_to_string(file)?.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let uuid: Uuid = line.parse()?;
        let now = chat::now_ms();
        let create = Some((chat::new_id(&mut rng), now));
        let Some(target) = store.send(store::MinecraftUser { uuid, create }).await?? else {
            continue;
        };
        if !muted.insert(target.id) {
            continue;
        }
        let punishment = moderation::Punishment {
            id: chat::new_id(&mut rng),
            kind: entity::punishment::Kind::Mute,
            user: Some(target.id),
            ip: None,
            reason: String::new(),
            issued_by: None,
            created_at: now,
            expires_at: None,
        };
        store
            .send(store::Persist(vec![store::Write::Punish(punishment.to_model())]))
            .await?;
        imported += 1;
    }
    println!("Muted {} users.", imported);
    Ok(())
}

async fn start_server(config: Config, database: &DbConfig) -> Result<()> {
    // every connection is a file; Docker and systemd default to a soft limit of 1024
    match rlimit::increase_nofile_limit(u64::MAX) {
        Ok(limit) => info!("Open file limit is {}", limit),
        Err(err) => warn!("Could not raise the open file limit: {}", err),
    }

    let filters = filter::Filters::new(&config.filter).map_err(|message| Error::Config { message })?;
    info!("Message filters: {:?}", filters.names());
    let store = open_store(database).await?;
    let moderation = moderation::Moderation::new(active_punishments(&store).await?);

    let server_config = config.clone();
    let server = chat::ChatServer::new(server_config, store, moderation, filters).start();

    let server_data = web::Data::new(server);
    let real_ip = web::Data::new(ip::RealIp::new(
        &config.net.trusted_proxies,
        config.net.real_ip_header.clone(),
    ));
    let net = config.net.clone();

    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(server_data.clone())
            .app_data(real_ip.clone())
            .service(web::resource("/ws").to(chat::chat_route))
    });

    for address in &net.address {
        server = server.bind(address)?;
    }

    if let (Some(cert), Some(key)) = (&net.cert_file, &net.key_file) {
        #[cfg(all(feature = "openssl-tls", feature = "rustls-tls"))]
        {
            compile_error!("Can't enable both the `openssl-tls` and the `rustls-tls` feature.")
        }

        #[cfg(feature = "openssl-tls")]
        for address in &net.tls_address {
            let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
            builder.set_certificate_chain_file(cert)?;
            let ft = match key.extension() {
                Some(ext) if ext == "pem" => SslFiletype::PEM,
                _ => SslFiletype::ASN1,
            };
            builder.set_private_key_file(key, ft)?;

            server = server.bind_openssl(address, builder)?;
        }

        #[cfg(feature = "rustls-tls")]
        {
            info!("Loading TLS certificate from {:?} and key from {:?}", cert, key);

            let cert_chain = CertificateDer::pem_file_iter(cert)
                .and_then(|certs| certs.collect::<std::result::Result<Vec<_>, _>>())
                .map_err(|_| Error::RustTLSNoMsg)?;
            let key = PrivateKeyDer::from_pem_file(key).map_err(|_| Error::RustTLSNoMsg)?;

            let config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(cert_chain, key)
                .map_err(|_| Error::RustTLSNoMsg)?;

            for address in &net.tls_address {
                server = server.bind_rustls_0_23(address, config.clone())?;
            }
        }

        #[cfg(not(any(feature = "openssl-tls", feature = "rustls-tls")))]
        warn!("Built without TLS support, not listening on {:?}", net.tls_address);

        info!("Serving HTTPS at {:?}", net.tls_address);
    }

    info!("Serving HTTP at {:?}", net.address);
    server.run().await?;
    Ok(())
}
