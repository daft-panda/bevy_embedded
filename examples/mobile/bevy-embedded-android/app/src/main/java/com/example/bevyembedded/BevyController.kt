package com.example.bevyembedded

/**
 * Controller interface for interacting with a Bevy app instance
 * Provides a clean Kotlin API similar to the iOS BevyViewController
 */
class BevyController(
    private val surfaceView: BevySurfaceView,
) {
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
}
