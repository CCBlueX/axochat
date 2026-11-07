package net.ccbluex.axochat.protocol

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerializationException
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.descriptors.element
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

/**
 * Travels as `{"type": "global"}` or `{"type": "direct", "user": "..."}`.
 */
@Serializable(with = ChannelSerializer::class)
public sealed interface Channel {

    public data object Global : Channel

    /**
     * Received with the id of the other side; sent with an id or an account name.
     */
    public data class Direct(val user: String) : Channel

    /**
     * A channel of a newer server.
     */
    public data class Unknown(val type: String) : Channel
}

internal object ChannelSerializer : KSerializer<Channel> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("Channel") {
        element<String>("type")
        element<String>("user", isOptional = true)
    }

    override fun serialize(encoder: Encoder, value: Channel) {
        val json = encoder as? JsonEncoder ?: throw SerializationException("Channels are JSON only")
        json.encodeJsonElement(buildJsonObject {
            when (value) {
                Channel.Global -> put("type", JsonPrimitive("global"))
                is Channel.Direct -> {
                    put("type", JsonPrimitive("direct"))
                    put("user", JsonPrimitive(value.user))
                }
                is Channel.Unknown -> put("type", JsonPrimitive(value.type))
            }
        })
    }

    override fun deserialize(decoder: Decoder): Channel {
        val json = decoder as? JsonDecoder ?: throw SerializationException("Channels are JSON only")
        val channel = json.decodeJsonElement() as? JsonObject
            ?: throw SerializationException("A channel is a JSON object")
        fun string(key: String) = (channel[key] as? JsonPrimitive)?.takeIf { it.isString }?.content
        return when (val type = string("type") ?: throw SerializationException("A channel has a type")) {
            "global" -> Channel.Global
            "direct" -> Channel.Direct(string("user") ?: throw SerializationException("A direct channel has a user"))
            else -> Channel.Unknown(type)
        }
    }
}
