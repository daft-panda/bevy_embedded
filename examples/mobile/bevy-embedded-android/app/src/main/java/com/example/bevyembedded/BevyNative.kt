package com.example.bevyembedded

import android.view.Surface

/**
 * Native JNI interface to Bevy Rust library
 */
object BevyNative {
    init {
        System.loadLibrary("bevy_mobile_embedded_example")
    }

    /**
     * Create a new Bevy app instance
     * @param surface The Android Surface to render to
     * @param width Surface width in pixels
     * @param height Surface height in pixels
     * @param scaleFactor Display density scale factor
     * @return Pointer to the Bevy app instance
     */
    external fun nativeCreateApp(
        surface: Surface,
        width: Int,
        height: Int,
        scaleFactor: Float
    ): Long

    /**
     * Update the Bevy app (render one frame)
     * @param appPtr Pointer to the Bevy app instance
     */
    external fun nativeUpdate(appPtr: Long)

    /**
     * Destroy the Bevy app instance and free resources
     * @param appPtr Pointer to the Bevy app instance
     */
    external fun nativeDestroy(appPtr: Long)

    /**
     * Send a touch event to Bevy
     * @param appPtr Pointer to the Bevy app instance
     * @param phase Touch phase (0=Started, 1=Moved, 2=Ended, 3=Canceled)
     * @param x X coordinate
     * @param y Y coordinate
     * @param id Touch pointer ID
     */
    external fun nativeTouchEvent(
        appPtr: Long,
        phase: Int,
        x: Float,
        y: Float,
        id: Long
    )

    /**
     * Notify Bevy of surface size changes
     * @param appPtr Pointer to the Bevy app instance
     * @param width New width in pixels
     * @param height New height in pixels
     * @param scaleFactor Display density scale factor
     */
    external fun nativeResize(
        appPtr: Long,
        width: Int,
        height: Int,
        scaleFactor: Float
    )

    /**
     * Send a message to Bevy
     * @param appPtr Pointer to the Bevy app instance
     * @param data Message data as byte array
     */
    external fun nativeSendMessage(appPtr: Long, data: ByteArray)

    /**
     * Receive a message from Bevy
     * @param appPtr Pointer to the Bevy app instance
     * @return Message data as byte array, or null if no message available
     */
    external fun nativeReceiveMessage(appPtr: Long): ByteArray?
}
