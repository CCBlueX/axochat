@file:UseSerializers(UuidSerializer::class)

package net.ccbluex.axochat.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.UseSerializers
import java.util.UUID

public sealed interface Serverbound {

    @Serializable
    @SerialName("Hello")
    public data class Hello(val protocol: Int) : Serverbound

    @Serializable
    @SerialName("RequestMojangInfo")
    public data object RequestMojangInfo : Serverbound

    /**
     * Sent after joining the session server with the hash from [Clientbound.MojangInfo].
     * After an account login it proves the Minecraft account instead of logging in.
     */
    @Serializable
    @SerialName("LoginMojang")
    public data class LoginMojang(val name: String, val uuid: UUID, val allowMessages: Boolean) : Serverbound

    @Serializable
    @SerialName("LoginAccount")
    public data class LoginAccount(val token: String, val allowMessages: Boolean) : Serverbound {
        override fun toString(): String = "LoginAccount(allowMessages=$allowMessages)"
    }

    @Serializable
    @SerialName("LoginJWT")
    public data class LoginJWT(val token: String, val allowMessages: Boolean) : Serverbound {
        override fun toString(): String = "LoginJWT(allowMessages=$allowMessages)"
    }

    @Serializable
    @SerialName("RequestJWT")
    public data object RequestJWT : Serverbound

    @Serializable
    @SerialName("Message")
    public data class Message(val content: String) : Serverbound

    @Serializable
    @SerialName("PrivateMessage")
    public data class PrivateMessage(val receiver: String, val content: String) : Serverbound

    /**
     * After [Hello]: [Channel.Direct] needs a LiquidBounce Account on both sides.
     */
    @Serializable
    @SerialName("ChatMessage")
    public data class ChatMessage(val channel: Channel, val content: String) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("BanUser")
    public data class BanUser(val user: UUID) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("UnbanUser")
    public data class UnbanUser(val user: UUID) : Serverbound

    /**
     * Staff only.
     */
    @Serializable
    @SerialName("RequestUserCount")
    public data object RequestUserCount : Serverbound
}
