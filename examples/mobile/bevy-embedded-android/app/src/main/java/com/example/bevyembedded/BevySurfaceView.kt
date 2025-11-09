package com.example.bevyembedded

import android.content.Context
import android.util.AttributeSet
import android.util.Log
import android.view.Choreographer
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android SurfaceView that hosts the Bevy engine
 */
class BevySurfaceView
    @JvmOverloads
    constructor(
        context: Context,
        attrs: AttributeSet? = null,
        defStyleAttr: Int = 0,
    ) : SurfaceView(context, attrs, defStyleAttr),
        SurfaceHolder.Callback {
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
        private val choreographer = Choreographer.getInstance()

        var onMessageReceived: ((ByteArray) -> Unit)? = null
        var onError: ((String) -> Unit)? = null
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

        override fun surfaceChanged(
            holder: SurfaceHolder,
            format: Int,
            width: Int,
            height: Int,
        ) {
            Log.d(TAG, "Surface changed: ${width}x$height @ ${scaleFactor}x scale")

            if (bevyAppPtr == 0L) {
                // First time - create the Bevy app
                setupBevy(holder.surface, width, height)
            } else {
                // Surface resized - notify Bevy and restart rendering if we were running
                BevyNative.nativeResize(bevyAppPtr, width, height, scaleFactor)
                if (!isRunning.get()) {
                    startRenderLoop()
                }
            }
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            Log.d(TAG, "Surface destroyed")
            // Only stop rendering, don't destroy Bevy - it might come back
            pauseRendering()
        }

        private fun setupBevy(
            surface: Surface,
            width: Int,
            height: Int,
        ) {
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
            Log.d(TAG, "Render loop started")

            // Schedule the first frame
            choreographer.postFrameCallback(frameCallback)
        }

        private val frameCallback =
            object : Choreographer.FrameCallback {
                override fun doFrame(frameTimeNanos: Long) {
                    if (!isRunning.get() || bevyAppPtr == 0L) {
                        Log.d(TAG, "Render loop stopped")
                        return
                    }

                    try {
                        // Update Bevy (renders one frame) - returns 0 on success
                        val errorCode = BevyNative.nativeUpdate(bevyAppPtr)

                        if (errorCode != 0) {
                            // Get the error message
                            val errorMessage = BevyNative.nativeGetLastError()
                                ?: "Bevy error (code: $errorCode)"

                            Log.e(TAG, "Bevy error: $errorMessage")

                            // Call error handler and stop rendering
                            onError?.invoke(errorMessage)

                            // Stop the render loop
                            stopBevy()
                            return
                        }

                        // Poll for messages from Bevy
                        pollBevyMessages()
                    } catch (e: Exception) {
                        Log.e(TAG, "Error in render loop", e)
                    }

                    // Schedule next frame
                    choreographer.postFrameCallback(this)
                }
            }

        private fun pollBevyMessages() {
            try {
                val message = BevyNative.nativeReceiveMessage(bevyAppPtr)
                if (message != null && message.isNotEmpty()) {
                    // Choreographer callbacks run on main thread, so we can invoke directly
                    onMessageReceived?.invoke(message)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Error polling messages", e)
            }
        }

        private fun pauseRendering() {
            if (isRunning.compareAndSet(true, false)) {
                Log.d(TAG, "Pausing rendering")
                choreographer.removeFrameCallback(frameCallback)
            }
        }

        private fun stopBevy() {
            pauseRendering()

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

            try {
                when (event.actionMasked) {
                    MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                        // Send event for the pointer that just went down
                        val pointerIndex = event.actionIndex
                        sendTouchEvent(PHASE_STARTED, event, pointerIndex)
                    }
                    MotionEvent.ACTION_MOVE -> {
                        // Send events for all active pointers
                        for (i in 0 until event.pointerCount) {
                            sendTouchEvent(PHASE_MOVED, event, i)
                        }
                    }
                    MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                        // Send event for the pointer that just went up
                        val pointerIndex = event.actionIndex
                        sendTouchEvent(PHASE_ENDED, event, pointerIndex)
                    }
                    MotionEvent.ACTION_CANCEL -> {
                        // Send cancel for all active pointers
                        for (i in 0 until event.pointerCount) {
                            sendTouchEvent(PHASE_CANCELED, event, i)
                        }
                    }
                    else -> return super.onTouchEvent(event)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Error sending touch event", e)
            }

            performClick()
            return true
        }

        override fun performClick(): Boolean {
            super.performClick()
            return true
        }

        private fun sendTouchEvent(phase: Int, event: MotionEvent, pointerIndex: Int) {
            val x = event.getX(pointerIndex)
            val y = event.getY(pointerIndex)
            val id = event.getPointerId(pointerIndex).toLong()
            BevyNative.nativeTouchEvent(bevyAppPtr, phase, x, y, id)
        }

        /**
         * Send a message to Bevy
         */
        fun sendMessage(data: ByteArray) {
            if (bevyAppPtr != 0L && isRunning.get()) {
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
