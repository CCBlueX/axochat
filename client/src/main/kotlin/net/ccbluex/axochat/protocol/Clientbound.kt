package net.ccbluex.axochat.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import net.ccbluex.axochat.user.Author
import net.ccbluex.axochat.user.LegacyUser
import net.ccbluex.axochat.user.Punishment
import net.ccbluex.axochat.user.ReportView
import net.ccbluex.axochat.user.UserRef

public sealed interface Clientbound {

    @Serializable
    @SerialName("Hello")
    public data class Hello(val protocol: Int) : Clientbound

    @Serializable
    @SerialName("MojangInfo")
    public data class MojangInfo(val sessionHash: String) : Clientbound

    @Serializable
    @SerialName("NewJWT")
    public data class NewJWT(val token: String) : Clientbound {
        override fun toString(): String = "NewJWT"
    }

    @Serializable
    @SerialName("Message")
    public data class Message(val authorInfo: LegacyUser, val content: String) : Clientbound

    @Serializable
    @SerialName("PrivateMessage")
    public data class PrivateMessage(val authorInfo: LegacyUser, val content: String) : Clientbound

    @Serializable
    @SerialName("UserCount")
    public data class UserCount(val connections: Int, val loggedIn: Int) : Clientbound

    @Serializable
    @SerialName("Success")
    public data class Success(val reason: SuccessReason) : Clientbound

    /**
     * Without [Serverbound.Hello], `InvalidCharacter` comes as `{"InvalidCharacter": "x"}`, hence the raw [message].
     */
    @Serializable
    @SerialName("Error")
    public data class Error(val message: JsonElement = JsonNull, val detail: String? = null) : Clientbound {
        val code: ErrorCode
            get() = when (message) {
                is JsonNull -> ErrorCode.Internal
                is JsonPrimitive -> ErrorCode(message.content)
                is JsonObject -> ErrorCode(message.keys.firstOrNull() ?: ErrorCode.Internal.code)
                else -> ErrorCode.Internal
            }

        val details: String?
            get() = detail ?: ((message as? JsonObject)?.values?.firstOrNull() as? JsonPrimitive)?.contentOrNull
    }

    /**
     * Sent before [Success] of a login, to clients that sent [Serverbound.Hello].
     */
    @Serializable
    @SerialName("Welcome")
    public data class Welcome(val user: Author, val staff: Boolean) : Clientbound

    /**
     * Replaces [Message] and [PrivateMessage] for clients that sent [Serverbound.Hello].
     *
     * @param channel for a direct message the other side: the sender, or the receiver in the echo to the sender
     * @param id counts up during a server run
     * @param time milliseconds since the Unix epoch
     * @param original staff only: what the author wrote, if the server censored [content]
     */
    @Serializable
    @SerialName("ChatMessage")
    public data class ChatMessage(
        val channel: Channel,
        val id: Long,
        val time: Long,
        val author: Author,
        val content: String,
        val original: String? = null,
    ) : Clientbound

    /**
     * A [ChatMessage] a server filter or staff took back after it was delivered; clients hide it or mark it as removed.
     *
     * @param reason staff only: why a filter removed it
     * @param muted staff only: milliseconds since the Unix epoch when the mute the author got for it ends
     */
    @Serializable
    @SerialName("MessageRemoved")
    public data class MessageRemoved(val id: Long, val reason: String? = null, val muted: Long? = null) : Clientbound

    /**
     * The receiver was just punished; a ban also ends the login.
     *
     * @param expires milliseconds since the Unix epoch, null if permanent
     */
    @Serializable
    @SerialName("Punished")
    public data class Punished(val kind: PunishmentKind, val reason: String, val expires: Long? = null) : Clientbound

    @Serializable
    @SerialName("Punishments")
    public data class Punishments(val user: UserRef, val punishments: List<Punishment>) : Clientbound

    /**
     * Sent to staff for every new report.
     */
    @Serializable
    @SerialName("ReportCreated")
    public data class ReportCreated(val report: ReportView) : Clientbound

    /**
     * The latest unresolved reports, newest first.
     */
    @Serializable
    @SerialName("Reports")
    public data class Reports(val reports: List<ReportView>) : Clientbound

    public data class Unknown(val name: String, val content: JsonElement?) : Clientbound
}
