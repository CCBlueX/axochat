@file:UseSerializers(UuidSerializer::class)

package net.ccbluex.axochat.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.UseSerializers
import net.ccbluex.axochat.moderation.PunishmentKind
import java.util.UUID

public sealed interface Serverbound {

    @Serializable
    @SerialName("Hello")
    public data class Hello(val protocol: Int) : Serverbound

    @Serializable
    @SerialName("RequestMojangInfo")
    public data object RequestMojangInfo : Serverbound

    /**
     * Also proves the Minecraft session after an account login. Sent after joining the session server
     * with the hash from [Clientbound.MojangInfo].
     */
    @Serializable
    @SerialName("LoginMojang")
    public data class LoginMojang(val name: String, val uuid: UUID, val allowMessages: Boolean) : Serverbound

    @Serializable
    @SerialName("LoginAccount")
    public data class LoginAccount(val token: String, val allowMessages: Boolean) : Serverbound {
        override fun toString(): String = "LoginAccount(allowMessages=$allowMessages)"
    }

    /**
     * Protocol v1; v2 sends [ChatMessage].
     */
    @Serializable
    @SerialName("Message")
    public data class Message(val content: String) : Serverbound

    /**
     * Protocol v1; v2 sends [ChatMessage].
     */
    @Serializable
    @SerialName("PrivateMessage")
    public data class PrivateMessage(val receiver: String, val content: String) : Serverbound

    @Serializable
    @SerialName("RequestUserCount")
    public data object RequestUserCount : Serverbound

    @Serializable
    @SerialName("Settings")
    public data class Settings(
        val allowMessages: Boolean? = null,
        val serverChat: Boolean? = null,
        /**
         * Staff only: every server's chat, on [Channel.Watched].
         */
        val watchServers: Boolean? = null,
    ) : Serverbound

    @Serializable
    @SerialName("ChatMessage")
    public data class ChatMessage(val channel: String, val content: String) : Serverbound {
        public constructor(channel: Channel, content: String) : this(channel.toString(), content)
    }

    @Serializable
    @SerialName("Report")
    public data class Report(val user: String, val message: Long? = null, val reason: String) : Serverbound

    @Serializable
    @SerialName("RequestReports")
    public data object RequestReports : Serverbound

    @Serializable
    @SerialName("ResolveReport")
    public data class ResolveReport(val id: String) : Serverbound

    /**
     * @param duration in seconds, `null` for permanent
     */
    @Serializable
    @SerialName("Punish")
    public data class Punish(
        val user: String? = null,
        val ip: String? = null,
        val kind: PunishmentKind,
        val duration: Long? = null,
        val reason: String,
        val includeIp: Boolean = false,
    ) : Serverbound

    @Serializable
    @SerialName("Pardon")
    public data class Pardon(val user: String? = null, val ip: String? = null) : Serverbound

    @Serializable
    @SerialName("RequestPunishments")
    public data class RequestPunishments(val user: String) : Serverbound

    /**
     * The Minecraft server the client plays on, `null` in singleplayer or outside a world.
     */
    @Serializable
    @SerialName("Location")
    public data class Location(val server: String? = null) : Serverbound
}
