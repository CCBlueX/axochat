package net.ccbluex.axochat.protocol

import net.ccbluex.axochat.user.UserKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class AxochatCodecTest {

    @Test
    fun `tokens never reach logs`() {
        assertEquals("LoginAccount(allowMessages=true)", Serverbound.LoginAccount("secret", true).toString())
    }

    @Test
    fun `v1 servers send error codes as objects`() {
        val error = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":{"InvalidCharacter":"y"}}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, error.code)
        assertEquals("y", error.details)
    }

    @Test
    fun `unknown values and fields from newer servers decode`() {
        val packet = assertIs<Clientbound.ChatMessage>(AxochatCodec.decode(
            """{"m":"ChatMessage","c":{"channel":"global","id":1,"time":0,"future":1,""" +
                """"author":{"id":"a","kind":"robot","name":"R","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5"},""" +
                """"content":"hi"}}""",
        ))
        assertEquals(UserKind("robot"), packet.author.kind)
    }
}
