package com.example.bevyembedded

/**
 * Controller interface for interacting with a Bevy app instance
 * Provides a clean Kotlin API similar to the iOS BevyViewController
 */
class BevyController(private val surfaceView: BevySurfaceView) {
    /**
     * Send a message to Bevy as raw bytes
     */
    fun sendMessage(data: ByteArray) {
        surfaceView.sendMessage(data)
    }

    /**
     * Send structured data to Bevy (convenience method)
     */
    fun sendBytes(vararg bytes: Byte) {
        sendMessage(bytes)
    }

    /**
     * Send a Float array to Bevy (useful for colors, vectors, etc.)
     */
    fun sendFloats(vararg floats: Float) {
        val bytes = ByteArray(floats.size * 4)
        floats.forEachIndexed { index, value ->
            val bits = value.toBits()
            bytes[index * 4] = bits.toByte()
            bytes[index * 4 + 1] = (bits shr 8).toByte()
            bytes[index * 4 + 2] = (bits shr 16).toByte()
            bytes[index * 4 + 3] = (bits shr 24).toByte()
        }
        sendMessage(bytes)
    }
}
