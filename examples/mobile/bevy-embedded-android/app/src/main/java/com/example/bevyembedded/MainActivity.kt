package com.example.bevyembedded

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import java.nio.ByteBuffer
import java.nio.ByteOrder

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            BevyEmbeddedApp()
        }
    }
}

@Composable
fun BevyEmbeddedApp() {
    var bevyController by remember { mutableStateOf<BevyController?>(null) }
    var cameraMatrix by remember { mutableStateOf("No camera data yet") }

    Box(modifier = Modifier.fillMaxSize()) {
        // Bevy render view
        AndroidView(
            factory = { context ->
                BevySurfaceView(context).apply {
                    bevyController = BevyController(this)

                    onMessageReceived = { data ->
                        cameraMatrix = handleBevyMessage(data)
                    }
                }
            },
            modifier = Modifier.fillMaxSize(),
        )

        // UI overlay
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            // Camera matrix display
            Text(
                text = cameraMatrix,
                color = Color.White,
                fontSize = 10.sp,
                fontFamily = FontFamily.Monospace,
                modifier =
                    Modifier
                        .background(
                            Color.Black.copy(alpha = 0.7f),
                            RoundedCornerShape(8.dp),
                        ).padding(12.dp),
            )

            Spacer(modifier = Modifier.weight(1f))

            // Color buttons
            Row(
                modifier =
                    Modifier
                        .background(
                            Color.Black.copy(alpha = 0.6f),
                            RoundedCornerShape(12.dp),
                        ).padding(12.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                ColorButton(
                    title = "Red",
                    color = Color.Red,
                    controller = bevyController,
                )
                ColorButton(
                    title = "Green",
                    color = Color.Green,
                    controller = bevyController,
                )
                ColorButton(
                    title = "Blue",
                    color = Color.Blue,
                    controller = bevyController,
                )
                ColorButton(
                    title = "Random",
                    color = Color(0xFF9C27B0), // Purple
                    controller = bevyController,
                    isRandom = true,
                )
            }
        }
    }
}

@Composable
fun ColorButton(
    title: String,
    color: Color,
    controller: BevyController?,
    isRandom: Boolean = false,
) {
    Button(
        onClick = {
            sendColor(controller, color, isRandom)
        },
        colors = ButtonDefaults.buttonColors(containerColor = color),
        modifier = Modifier.height(40.dp),
    ) {
        Text(
            text = title,
            color = Color.White,
        )
    }
}

fun sendColor(
    controller: BevyController?,
    color: Color,
    isRandom: Boolean,
) {
    val rgba =
        if (isRandom) {
            // Generate random color
            floatArrayOf(
                kotlin.random.Random.nextFloat(),
                kotlin.random.Random.nextFloat(),
                kotlin.random.Random.nextFloat(),
                1.0f,
            )
        } else {
            // Use provided color
            floatArrayOf(color.red, color.green, color.blue, color.alpha)
        }

    // Pack Vec4 as 16 bytes (4 x f32 in little-endian)
    val buffer = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
    rgba.forEach { buffer.putFloat(it) }

    controller?.sendMessage(buffer.array())
}

fun handleBevyMessage(data: ByteArray): String =
    try {
        when (data.size) {
            64 -> {
                // Camera matrix (Mat4 = 16 floats = 64 bytes)
                val buffer = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
                val floats = FloatArray(16) { buffer.getFloat() }

                """Camera Mat4:
[%.2f %.2f %.2f %.2f]
[%.2f %.2f %.2f %.2f]
[%.2f %.2f %.2f %.2f]
[%.2f %.2f %.2f %.2f]""".format(
                    floats[0],
                    floats[1],
                    floats[2],
                    floats[3],
                    floats[4],
                    floats[5],
                    floats[6],
                    floats[7],
                    floats[8],
                    floats[9],
                    floats[10],
                    floats[11],
                    floats[12],
                    floats[13],
                    floats[14],
                    floats[15],
                )
            }
            else -> "Received ${data.size} bytes"
        }
    } catch (e: Exception) {
        "Error parsing message: ${e.message}"
    }
