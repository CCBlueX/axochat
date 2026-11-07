# The AxoChat protocol
The AxoChat protocol is based on websockets.
All packets are sent to the `/ws` endpoint.

<!-- markdown-toc start - Don't edit this section. Run M-x markdown-toc-refresh-toc -->
**Table of Contents**

- [The AxoChat protocol](#the-axochat-protocol)
- [Structures](#structures)
    - [Id](#id)
    - [UserInfo](#userinfo)
- [Handshake](#handshake)
- [Minecraft proof](#minecraft-proof)
- [Packets](#packets)
    - [Client](#client)
        - [Error](#error)
        - [Hello](#hello)
        - [Message](#message)
        - [MojangInfo](#mojanginfo)
        - [NewJWT](#newjwt)
        - [PrivateMessage](#privatemessage)
        - [Success](#success)
        - [UserCount](#usercount)
        - [Welcome](#welcome)
    - [Server](#server)
        - [BanUser](#banuser)
        - [Hello](#hello-1)
        - [LoginAccount](#loginaccount)
        - [LoginJWT](#loginjwt)
        - [LoginMojang](#loginmojang)
        - [Message](#message-1)
        - [PrivateMessage](#privatemessage-1)
        - [RequestJWT](#requestjwt)
        - [RequestMojangInfo](#requestmojanginfo)
        - [RequestUserCount](#requestusercount)
        - [UnbanUser](#unbanuser)

<!-- markdown-toc end -->

# Handshake
A client may send [Hello](#hello-1) before logging in to announce the newest protocol it speaks.
The server answers with [Hello](#hello) and the protocol both sides use.
Clients that announce protocol 2 or newer get the packets added since, such as [Welcome](#welcome),
and [Error](#error) packets with a `detail`.
Clients that send no `Hello` keep the original protocol, and servers that do not know `Hello` ignore it,
so a client that gets no answer within a few seconds uses the original protocol.

# Minecraft proof
After [LoginAccount](#loginaccount), a session can prove the Minecraft account it plays on,
with the same packets as a Mojang login:
[RequestMojangInfo](#requestmojanginfo), joining the session server with the hash from [MojangInfo](#mojanginfo),
then [LoginMojang](#loginmojang) (`allow_messages` is ignored).
The server answers with [Welcome](#welcome) (after a [handshake](#handshake)), now showing the account as `minecraft`,
and [Success](#success) with `Minecraft`; or with an [Error](#error), such as `LoginFailed` or `InvalidId`,
and the session stays logged in as before.
The proof lasts until the session ends.

# Packets
Packets are sent in websocket `text` messages encoded as JSON objects.
They all have a structure like that, with `c` being optional:
```json
{
    "m": "Name",
    "c": {
        "...": "...",
        "...": false
    }
}
```

Not every packet has a body:
```json
{
    "m": "Name"
}
```

## Client
Client Packets are received by the client.

### Error
This packet may be sent at any time,
but is usually a response to a failed action of the client.

- `message` is the error code.
- `detail` is only sent after a [handshake](#handshake), and only for errors that have one,
  like the character of `InvalidCharacter`.

Without a handshake, `InvalidCharacter` is sent as an object holding the character:
`"message": {"InvalidCharacter": "§"}`.

**Example**
```json
{
    "m": "Error",
    "c": {
        "message": "LoginFailed"
    }
}
```

### Hello
The answer to [Hello](#hello-1).

- `protocol` is the protocol the server uses for this connection, at most the one the client announced.

**Example**
```json
{
    "m": "Hello",
    "c": {
        "protocol": 2
    }
}
```

### Message
This packet will be sent to every authenticated client,
if another client successfully [sent a message](#message-1) to the server.

- `author_info` is just the name and uuid of the user that sent the message.
  For a LiquidBounce Account, `uuid` is its linked Minecraft account, or the nil uuid if there is none.
- `content` is any message fitting the validation scheme of the server.
  Staff and users with a perk role may also use `§` color and formatting codes (except `§k`) and emoji.

**Example**
```json
{
    "m": "Message",
    "c": {
        "author_info": {
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        },
        "content": "Hello, World!"
    }
}
```

### MojangInfo
After the client sent the server a [RequestMojangInfo](#requestmojanginfo)
packet, the server will provide the client with a `session_hash`.
A session hash is synonymous with a *server id* in the context of
[authentication with mojang](https://wiki.vg/Protocol_Encryption#Authentication).
The client has to send a [LoginMojang](#loginmojang) packet to the server
after authenticating itself with mojang.

**Example**
```json
{
    "m": "MojangInfo",
    "c": {
        "session_hash": "88e16a1019277b15d58faf0541e11910eb756f6"
    }
}
```

### NewJWT
After the client sent the server a [RequestJWT](#requestjwt)
packet, the server will provide the client with json web token.
This token can be used in the [LoginJWT](#loginjwt) packet.

**Example**
```json
{
    "m": "NewJWT",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhIGpzb24gd2ViIHRva2VuLCBidXQgaXQgaXNuJ3QK"
    }
}
```

### PrivateMessage
The content of this packet will be sent to a authenticated client with `allow_messages` turned on,
if another client successfully [sent a private message](#privatemessage-1).

- `author_info` is just the name and uuid of the user that sent the message.
- `content` is any message fitting the validation scheme of the server.

**Example**
```json
{
    "m": "PrivateMessage",
    "c": {
        "author_info": {
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        },
        "content": "Hello, User!"
    }
}
```

### Success
This packet is sent after either
[LoginMojang](#loginmojang), [LoginJWT](#loginjwt), [LoginAccount](#loginaccount),
[BanUser](#banuser) or [UnbanUser](#unbanuser)
were processed successfully.

- `reason` is the reason for the success; it is one of the following possible
  values:
  - `Login`
  - `Ban`
  - `Unban`
  - `Minecraft` after a [Minecraft proof](#minecraft-proof)

**Example**
```json
{
    "m": "Success",
    "c": {
        "reason": "Login"
    }
}
```

### UserCount
This packet is sent after [RequestUserCount](#requestusercount) was received.

- `connections` is the amount of connections this server has open
- `logged_in` is the amount of authenticated connections this server has open

**Example**
```json
{
    "m": "UserCount",
    "c": {
        "connections": 623,
        "logged_in": 531,
    }
}
```

### Welcome
Sent after a successful login, right before [Success](#success), to clients after a [handshake](#handshake).

- `user` is the logged in user:
  - `id` is the id of the user on this server.
  - `kind` is `account` for a LiquidBounce Account and `mojang` for a Minecraft account without one.
  - `name` is the name others see.
  - `uuid` is the Minecraft account shown for the user, the nil uuid if unknown.
  - `minecraft` is the `name` and `uuid` of the Minecraft account the session proved it plays on,
    left out without a [Minecraft proof](#minecraft-proof).
  - `roles` are the roles of the account, staff roles first: `id`, display `name` and whether it is a `staff` role.
- `staff` tells whether the user may moderate.

Clients should ignore fields and values they do not know.

**Example**
```json
{
    "m": "Welcome",
    "c": {
        "user": {
            "id": "0190c3a2-5f1e-7c4b-9d2a-1b3c4d5e6f70",
            "kind": "account",
            "name": "Notch",
            "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
            "roles": [
                { "id": "premium", "name": "Premium", "staff": false }
            ]
        },
        "staff": false
    }
}
```

## Server
Server Packets are received by the server.

### BanUser
Staff can send this packet to ban other users from this chat.
Banned users are logged out and cannot log in again; they get an `Error` with `Banned`.

- `user` is the uuid of the user to ban.

**Example**
```json
{
    "m": "BanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```

### Hello
Starts the [handshake](#handshake). Only answered before logging in.

- `protocol` is the newest protocol the client speaks.

**Example**
```json
{
    "m": "Hello",
    "c": {
        "protocol": 2
    }
}
```

### LoginAccount
Logs in with a LiquidBounce Account.
The server answers with [Welcome](#welcome) (after a [handshake](#handshake)) and [Success](#success),
or an [Error](#error): `LoginFailed` for an invalid token, `Banned` for a banned user.

- `token` is an OAuth access token of the LiquidBounce Service API.
- If `allow_messages` is true, other clients may send private messages
  to this client.

**Example**
```json
{
    "m": "LoginAccount",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhbiBhY2Nlc3MgdG9rZW4K",
        "allow_messages": true
    }
}
```

### LoginJWT
To login using a json web token, the client has to send a `LoginJWT` packet.
it will send [Success](#success) if the login was successful.

- `token` can be retrieved by sending [RequestJWT](#requestjwt) on an already
- authenticated connection.
- If `allow_messages` is true, other clients may send private messages
  to this client.

**Example**
```json
{
    "m": "LoginJWT",
    "c": {
        "token": "VGhpcyBjb3VsZCBiZSBhIGpzb24gd2ViIHRva2VuLCBidXQgaXQgaXNuJ3QK",
        "allow_messages": true,
    }
}
```

### LoginMojang
After the client received a [MojangInfo](#mojanginfo) packet
and authenticating itself with mojang,
it has to send a `LoginMojang` packet to the server.
After the server receives a `LoginMojang` packet,
it will send [Success](#success) if the login was successful.
A Minecraft account linked to a LiquidBounce Account logs in as that account; this applies to [LoginJWT](#loginjwt) too.
Sessions logged in with [LoginAccount](#loginaccount) use it for a [Minecraft proof](#minecraft-proof) instead.

- `name` needs to be associated with the uuid.
- `uuid` is not guaranteed to be hyphenated.
- If `allow_messages` is true, other clients may send private messages
  to this client.

**Example**
```json
{
    "m": "LoginMojang",
    "c": {
        "name": "Notch",
        "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "allow_messages": true
   }
}
```

### Message
The `content` of this packet will be sent to every client
as [Message](#message) if it fits the validation scheme.

**Example**
```json
{
    "m": "Message",
    "c": {
        "content": "Hello, World!"
    }
}
```

### PrivateMessage
The `content` of this packet will be sent to the specified client
as [PrivateMessage](#privatemessage) if it fits the validation scheme.

- `receiver` is the name of the receiver.

**Example**
```json
{
    "m": "PrivateMessage",
    "c": {
        "content": "Hello, Notch!",
        "receiver": "Notch"
    }
}
```

### RequestJWT
To login using [LoginJWT](#loginjwt), a client needs to own a json web token.
This token can be retrieved by sending `RequestJWT` as an already authenticated
client to the server.
The server will send a [NewJWT](#newjwt) packet to the client.
The token stands for a Minecraft account, so after [LoginAccount](#loginaccount) the server sends `NotSupported` instead.

This packet has no body.

**Example**
```json
{
    "m": "RequestJWT"
}
```

### RequestMojangInfo
To login via mojang, the client has to send a `RequestMojangInfo` packet.
The server will then send a [MojangInfo](#mojanginfo) to the client.

This packet has no body.

**Example**
```json
{
    "m": "RequestMojangInfo"
}
```

### RequestUserCount
After receiving this packet, the server will then send a [UserCount](#usercount)
packet to the client.

This packet has no body.

**Example**
```json
{
    "m": "RequestUserCount"
}
```

### UnbanUser
Staff can send this packet to lift every ban and mute of a user.

- `user` is the uuid of the user to unban.

**Example**
```json
{
    "m": "UnbanUser",
    "c": {
        "user": "069a79f4-44e9-4726-a5be-fca90e38aaf5"
    }
}
```
