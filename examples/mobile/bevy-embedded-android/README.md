# Bevy Embedded for Android

A complete example showing how to embed the Bevy game engine into a native Android application using Jetpack Compose.

## Features

✨ **Clean API**: Trait-based Rust API and Kotlin-friendly controller
🔄 **Bidirectional Messaging**: Send data between Kotlin and Bevy
🎮 **Touch Input**: Full touch event support
📱 **Universal**: Supports devices and emulators (ARM64, ARMv7, x86_64)
⚡ **Automatic Builds**: Gradle automatically compiles Rust during build

## Quick Start

### 1. Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# Install cargo-ndk (for building Android native libraries)
cargo install cargo-ndk

# Set up Android NDK (if not already set)
export ANDROID_NDK_ROOT=$HOME/Android/Sdk/ndk/26.1.10909125  # Adjust version as needed
```

### 2. Project Structure

```
your-android-app/
├── app/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── java/com/example/yourapp/
│       │   ├── MainActivity.kt          # Your Compose UI
│       │   ├── BevySurfaceView.kt       # Copy from this example
│       │   ├── BevyController.kt        # Copy from this example
│       │   └── BevyNative.kt            # Copy from this example
│       └── jniLibs/                     # Built Rust libraries go here
│           ├── arm64-v8a/
│           ├── armeabi-v7a/
│           └── x86_64/
└── bevy-project/
    ├── Cargo.toml
    ├── src/lib.rs                       # Your Bevy app
    └── build_android.sh                 # Build script
```

### 3. Create Your Bevy App

In `src/lib.rs`:

```rust
use bevy::prelude::*;
use bevy_embedded::{export_embedded_app, prelude::*};

struct MyApp;

impl EmbeddedApp for MyApp {
    fn setup(app: &mut App) {
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    ..Default::default()
                })
        )
        .add_systems(Startup, setup)
        .add_systems(Update, update);
    }
}

export_embedded_app!(MyApp);

fn setup(mut commands: Commands) {
    // Your setup code
}

fn update() {
    // Your update code
}
```

### 4. Copy Required Kotlin Files

Copy these files from this example to your project:

1. **BevySurfaceView.kt** - The main view that hosts Bevy
2. **BevyController.kt** - Controller for interacting with Bevy
3. **BevyNative.kt** - JNI interface definitions

Update the package names to match your app.

### 5. Configure Gradle

In your app's `build.gradle.kts`:

```kotlin
android {
    // ... other config ...

    // Custom task to build Rust library
    task("buildRustLibrary") {
        doLast {
            exec {
                workingDir = file("../../bevy-project")
                commandLine("bash", "build_android.sh")
            }
        }
    }

    // Make Rust library build run before native libraries are packaged
    tasks.named("preBuild") {
        dependsOn("buildRustLibrary")
    }
}

dependencies {
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    // ... other dependencies ...
}
```

### 6. Use in Jetpack Compose

```kotlin
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

@Composable
fun MyApp() {
    var bevyController by remember { mutableStateOf<BevyController?>(null) }

    AndroidView(
        factory = { context ->
            BevySurfaceView(context).apply {
                bevyController = BevyController(this)

                onMessageReceived = { data ->
                    println("Received from Bevy: ${data.size} bytes")
                }
            }
        },
        modifier = Modifier.fillMaxSize()
    )
}
```

### 7. Build and Run

#### Option A: Using Gradle (Automatic)

Just build in Android Studio (Ctrl+F9 or Build > Make Project). The Rust library compiles automatically.

#### Option B: Manual Build

```bash
cd bevy-project
./build_android.sh release   # or 'debug' for debug builds
```

Then build the Android app in Android Studio.

## Examples

### Sending Messages to Bevy

```kotlin
// Send raw bytes
bevyController?.sendMessage(byteArrayOf(1, 2, 3, 4))

// Send floats (e.g., color)
bevyController?.sendFloats(1.0f, 0.0f, 0.0f, 1.0f)  // Red color
```

### Receiving Messages in Bevy

```rust
fn my_system(channel: Res<HostChannel>) {
    while let Some(message) = channel.receive() {
        // Process message bytes
        info!("Received {} bytes", message.len());
    }
}
```

### Sending Messages from Bevy

```rust
fn my_system(channel: Res<HostChannel>) {
    let data = vec![1, 2, 3, 4];
    channel.send(data);
}
```

### Receiving in Kotlin

```kotlin
BevySurfaceView(context).apply {
    onMessageReceived = { data ->
        if (data.size == 64) {
            // Parse Mat4 or other structured data
            val buffer = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
            val floats = FloatArray(16) { buffer.getFloat() }
        }
    }
}
```

## Architecture

### Rust Side

- **`bevy_embedded` crate**: Core embedding functionality
  - `EmbeddedPlugin`: Replaces WinitPlugin for embedded mode
  - `HostChannel`: Bidirectional message passing
  - `EmbeddedApp` trait: Clean API for defining your app

- **Your app crate**: Implements `EmbeddedApp` trait
  - Uses `export_embedded_app!` macro to generate FFI entry points
  - No manual FFI code needed!

### Kotlin/Android Side

- **`BevySurfaceView`**: Android view that hosts Bevy (extends SurfaceView)
- **`BevyController`**: Clean Kotlin API for controlling Bevy
- **`BevyNative`**: JNI interface (hidden from users)
- **FFI layer**: Handled automatically via JNI

## Build Script

### `build_android.sh`

Builds the Rust library for all Android architectures:

```bash
./build_android.sh          # Release build (default)
./build_android.sh debug    # Debug build
```

Automatically:
- Detects and uses Android NDK
- Builds for ARM64, ARMv7, and x86_64
- Copies libraries to `app/src/main/jniLibs/`
- Handles Debug/Release configurations

## Performance

- **First build**: Slow (~5-10 min) as Bevy compiles
- **Incremental builds**: Fast (<30 sec) only changed code recompiles
- **Release builds**: Highly optimized, 60fps easily achievable
- **Binary size**: ~50-100MB for debug, ~10-20MB for release (after stripping)

## Tips

### Faster Debug Builds

Add to `~/.cargo/config.toml`:
```toml
[profile.dev]
opt-level = 1  # Slight optimization for better debug performance
```

### Smaller Release Builds

In `Cargo.toml`:
```toml
[profile.release]
strip = true  # Strip symbols
lto = true    # Link-time optimization
```

### Logging

Bevy logs are visible in Android Logcat. Configure in your app:
```rust
.set(LogPlugin {
    level: Level::INFO,
    filter: "wgpu=error,bevy_render=info".to_string(),
    ..Default::default()
})
```

View logs in Android Studio: View > Tool Windows > Logcat

## Troubleshooting

### "ANDROID_NDK_ROOT not set"

**Solution**: Set the environment variable:
```bash
export ANDROID_NDK_ROOT=$HOME/Android/Sdk/ndk/26.1.10909125
# Add to ~/.bashrc or ~/.zshrc for persistence
```

### "cargo-ndk: command not found"

**Solution**: Install cargo-ndk:
```bash
cargo install cargo-ndk
```

### "Library not found" when building

**Problem**: The build script couldn't find the compiled library.

**Solution**:
1. Check that the build completed without errors
2. Verify the target directory: `../../target/[arch]/[profile]/lib*.so`
3. Make sure your `Cargo.toml` has `crate-type = ["cdylib"]` or `["lib", "cdylib"]`

### App crashes on startup

**Common causes**:
1. **Missing JNI library**: Check that `.so` files are in `app/src/main/jniLibs/`
2. **Wrong package name**: JNI function names must match your package (e.g., `Java_com_example_yourapp_BevyNative_...`)
3. **NDK version mismatch**: Try using NDK 25 or 26

**Debug steps**:
```bash
# Check if library is included in APK
unzip -l app/build/outputs/apk/debug/app-debug.apk | grep libbevy

# View native crash logs
adb logcat | grep -i "native\|crash\|fatal"
```

### Build is very slow

**First build**: Normal - Bevy is a large framework
**Subsequent builds**: Should be fast (<30s). If not:
1. Use `sccache` for caching: `cargo install sccache`
2. Add to `~/.cargo/config.toml`:
   ```toml
   [build]
   rustc-wrapper = "sccache"
   ```

### Surface not rendering

**Check**:
1. Surface callbacks are being called (add logs in `surfaceCreated`, `surfaceChanged`)
2. Bevy app is created successfully (check logs for "Bevy app created")
3. Render loop is running (add logs in `BevySurfaceView` render thread)

## System Requirements

- **Minimum Android API**: 24 (Android 7.0)
- **Recommended API**: 29+ (Android 10+)
- **NDK Version**: 25 or 26 recommended
- **Rust**: 1.75+ (for Bevy 0.17)

## License

This example is dual-licensed under MIT or Apache 2.0, matching Bevy's license.
