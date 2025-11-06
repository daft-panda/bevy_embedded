package com.example.bevyembedded

import android.content.Context
import android.util.AttributeSet
import android.util.Log
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * Android SurfaceView that hosts the Bevy engine
 * This is the Android equivalent of iOS's BevyMetalView
 */
class BevySurfaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr), SurfaceHolder.Callback {

    companion object {
        private const val TAG = "BevySurfaceView"

        // Touch phase constants matching Rust
        private const val PHASE_STARTED = 0
        private const val PHASE_MOVED = 1
        private const val PHASE_ENDED = 2
        private const val PHASE_CANCELED = 3
    }

    private var bevyAppPtr: Long = 0
    private val isRunning = AtomicBoolean(false)
    private var renderThread: Thread? = null

    var onMessageReceived: ((ByteArray) -> Unit)? = null
    private val scaleFactor: Float = context.resources.displayMetrics.density

    init {
        holder.addCallback(this)
        // Enable touch events
        isClickable = true
        isFocusable = true
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        Log.d(TAG, "Surface created")
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        Log.d(TAG, "Surface changed: ${width}x${height} @ ${scaleFactor}x scale")

        if (bevyAppPtr == 0L) {
            // First time - create the Bevy app
            setupBevy(holder.surface, width, height)
        } else {
            // Surface resized - notify Bevy
            BevyNative.nativeResize(bevyAppPtr, width, height, scaleFactor)
        }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Log.d(TAG, "Surface destroyed")
        stopBevy()
    }

    private fun setupBevy(surface: Surface, width: Int, height: Int) {
        Log.d(TAG, "Setting up Bevy...")

        try {
            bevyAppPtr = BevyNative.nativeCreateApp(surface, width, height, scaleFactor)

            if (bevyAppPtr != 0L) {
                Log.d(TAG, "Bevy app created successfully: $bevyAppPtr")
                startRenderLoop()
            } else {
                Log.e(TAG, "Failed to create Bevy app")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error creating Bevy app", e)
        }
    }

    private fun startRenderLoop() {
        isRunning.set(true)

        renderThread = thread(start = true, name = "BevyRenderThread") {
            Log.d(TAG, "Render loop started")

            while (isRunning.get() && bevyAppPtr != 0L) {
                try {
                    // Update Bevy (renders one frame)
                    BevyNative.nativeUpdate(bevyAppPtr)

                    // Poll for messages from Bevy
                    pollBevyMessages()

                    // Limit to ~60 FPS
                    Thread.sleep(16)
                } catch (e: InterruptedException) {
                    Log.d(TAG, "Render loop interrupted")
                    break
                } catch (e: Exception) {
                    Log.e(TAG, "Error in render loop", e)
                }
            }

            Log.d(TAG, "Render loop stopped")
        }
    }

    private fun pollBevyMessages() {
        try {
            val message = BevyNative.nativeReceiveMessage(bevyAppPtr)
            if (message != null && message.isNotEmpty()) {
                // Call callback on main thread
                post {
                    onMessageReceived?.invoke(message)
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error polling messages", e)
        }
    }

    private fun stopBevy() {
        isRunning.set(false)

        renderThread?.apply {
            interrupt()
            join(1000) // Wait up to 1 second
        }
        renderThread = null

        if (bevyAppPtr != 0L) {
            try {
                BevyNative.nativeDestroy(bevyAppPtr)
                Log.d(TAG, "Bevy app destroyed")
            } catch (e: Exception) {
                Log.e(TAG, "Error destroying Bevy app", e)
            }
            bevyAppPtr = 0
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (bevyAppPtr == 0L) return super.onTouchEvent(event)

        val phase = when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> PHASE_STARTED
            MotionEvent.ACTION_MOVE -> PHASE_MOVED
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> PHASE_ENDED
            MotionEvent.ACTION_CANCEL -> PHASE_CANCELED
            else -> return super.onTouchEvent(event)
        }

        val pointerIndex = event.actionIndex
        val x = event.getX(pointerIndex)
        val y = event.getY(pointerIndex)
        val id = event.getPointerId(pointerIndex).toLong()

        try {
            BevyNative.nativeTouchEvent(bevyAppPtr, phase, x, y, id)
        } catch (e: Exception) {
            Log.e(TAG, "Error sending touch event", e)
        }

        return true
    }

    /**
     * Send a message to Bevy
     */
    fun sendMessage(data: ByteArray) {
        if (bevyAppPtr != 0L) {
            try {
                BevyNative.nativeSendMessage(bevyAppPtr, data)
            } catch (e: Exception) {
                Log.e(TAG, "Error sending message", e)
            }
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        stopBevy()
    }
}
