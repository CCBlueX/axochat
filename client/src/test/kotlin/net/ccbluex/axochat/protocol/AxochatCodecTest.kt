package net.ccbluex.axochat.protocol

import net.ccbluex.axochat.user.UserKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs

class AxochatCodecTest {

    @Test
    fun `tokens never reach logs`() {
        assertFalse("secret" in Serverbound.LoginAccount("secret", true).toString())
        assertFalse("secret" in Serverbound.LoginJWT("secret", true).toString())
        assertFalse("secret" in Clientbound.NewJWT("secret").toString())
    }

    @Test
    fun `errors without hello come as objects`() {
        val error = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":{"InvalidCharacter":"y"}}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, error.code)
        assertEquals("y", error.details)

        val detailed = assertIs<Clientbound.Error>(
            AxochatCodec.decode("""{"m":"Error","c":{"message":"InvalidCharacter","detail":"y"}}"""),
        )
        assertEquals(ErrorCode.InvalidCharacter, detailed.code)
        assertEquals("y", detailed.details)
    }

    @Test
    fun `unknown packets, values and fields decode`() {
        val welcome = assertIs<Clientbound.Welcome>(AxochatCodec.decode(
            """{"m":"Welcome","c":{"staff":false,"future":1,""" +
                """"user":{"id":"a","kind":"robot","name":"R","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5"}}}""",
        ))
        assertEquals(UserKind("robot"), welcome.user.kind)
        assertIs<Clientbound.Unknown>(AxochatCodec.decode("""{"m":"Later","c":{"x":1}}"""))
    }
}
