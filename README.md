# AxoChat Server
A generic server for chat features in Minecraft modifications utilizing the Mojang authentication scheme and WebSockets. LiquidBounce employs it for its global chat feature, which allows users to communicate with other people using the client regardless of the current server.

## Implementation
A specification of the protocol used can be found [here](PROTOCOL.md).

## Usage
The server is configured through environment variables, see [.env.example](.env.example).

```sh
cargo run --release -- start
```

It needs a MySQL or MariaDB database; tables are created on startup.
Staff are the users with a staff role in the LiquidBounce Service API (`API_URL`, `API_TOKEN`).
The Docker image runs in `/data`, where the certificates are kept; mount a volume there.
`docker compose up -d` runs it from `.env`; `COMPOSE_PROFILES=tunnel` and `TUNNEL_TOKEN` add a Cloudflare tunnel.

`axochat generate <name> [uuid]` prints a JWT for testing.
Mutes from an old `banned.txt` move into the database with `axochat import-bans banned.txt`.

## Kotlin client
`client/` is a Kotlin library for the protocol, published to `https://maven.ccbluex.net` as `net.ccbluex:axochat-client`.
Snapshots come from `master`, releases from `client-v<version>` tags.
