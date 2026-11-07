@file:UseSerializers(UuidSerializer::class)

package net.ccbluex.axochat.user

import kotlinx.serialization.Serializable
import kotlinx.serialization.UseSerializers
import net.ccbluex.axochat.protocol.UuidSerializer
import java.util.UUID

/**
 * [Mojang] is a Minecraft account without a LiquidBounce Account.
 */
@Serializable
@JvmInline
public value class UserKind(public val name: String) {
    override fun toString(): String = name

    public companion object {
        public val Account: UserKind = UserKind("account")
        public val Mojang: UserKind = UserKind("mojang")
    }
}

@Serializable
public data class Role(val id: String, val name: String, val staff: Boolean)

/**
 * @param uuid the Minecraft account shown as their head, nil if unknown
 * @param roles staff roles first
 */
@Serializable
public data class Author(
    val id: String,
    val kind: UserKind,
    val name: String,
    val uuid: UUID,
    val roles: List<Role> = emptyList(),
) {
    val isAccount: Boolean get() = kind == UserKind.Account
}

/**
 * The author of [net.ccbluex.axochat.protocol.Clientbound.Message] and
 * [net.ccbluex.axochat.protocol.Clientbound.PrivateMessage].
 */
@Serializable
public data class LegacyUser(val name: String, val uuid: UUID)
