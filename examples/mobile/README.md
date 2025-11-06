# Bevy Embedded for Mobile (iOS & Android)

Complete examples showing how to embed the Bevy game engine into native mobile applications.

## Examples

- **[iOS (SwiftUI)](./test-bevy-embedded/)** - iOS app using SwiftUI and Metal
- **[Android (Jetpack Compose)](./bevy-embedded-android/)** - Android app using Jetpack Compose and Vulkan

---

## iOS Example

A complete example showing how to embed the Bevy game engine into a native iOS SwiftUI application.

## Features

✨ **Clean API**: Trait-based Rust API and SwiftUI-friendly view controller
🔄 **Bidirectional Messaging**: Send data between Swift and Bevy
🎮 **Touch Input**: Full touch event support
📱 **Universal**: Supports both simulator and device builds
⚡ **Automatic Builds**: Xcode automatically compiles Rust during build

## Quick Start

### 1. Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add iOS targets
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

### 2. Project Structure

```
your-ios-app/
├── YourApp.xcodeproj
├── YourApp/
│   ├── BevyMetalView.swift      # Copy from this example
│   └── ContentView.swift         # Your SwiftUI views
└── bevy-project/
    ├── Cargo.toml
    ├── src/lib.rs               # Your Bevy app
    └── build_rust_deps.sh       # Copy from this example
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

### 4. Configure Xcode

See [XCODE_SETUP.md](./XCODE_SETUP.md) for detailed instructions.

**Quick version:**

1. Add build script phase (before "Compile Sources"):
   ```bash
   bash "${PROJECT_DIR}/../bevy-project/build_rust_deps.sh"
   ```

2. Add build setting:
   - Name: `BEVY_RUST_PROJECT_PATH`
   - Value: `$(PROJECT_DIR)/../bevy-project`

3. Embed the dylib in **General** tab → **Frameworks, Libraries, and Embedded Content** (set to "Embed & Sign")

4. Link required frameworks: Metal, MetalKit, UIKit, QuartzCore

### 5. Use in SwiftUI

```swift
import SwiftUI

struct ContentView: View {
    @State private var bevyController: BevyViewController?

    var body: some View {
        BevyMetalView(
            controller: $bevyController,
            onMessageReceived: { data in
                print("Received from Bevy: \(data)")
            }
        )
        .ignoresSafeArea()
    }
}
```

### 6. Build and Run

Just build in Xcode (⌘B)! The Rust library compiles automatically.

## Examples

### Sending Messages to Bevy

```swift
// Send bytes
bevyController?.sendBytes([1, 2, 3, 4])

// Send Data
let color = Color.red
bevyController?.sendMessage(colorData)
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

### Receiving in Swift

```swift
BevyMetalView(
    controller: $bevyController,
    onMessageReceived: { data in
        if data.count == 64 {
            // Parse Mat4 or other structured data
            let floats = data.withUnsafeBytes { ptr in
                Array(ptr.bindMemory(to: Float.self))
            }
        }
    }
)
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

### Swift Side

- **`BevyMetalView`**: SwiftUI view that hosts Bevy
- **`BevyViewController`**: Clean Swift API for controlling Bevy
- **FFI layer**: Hidden from users, handled automatically

## Scripts

### `build_rust_deps.sh`

Called by Xcode during build. Automatically:
- Detects simulator vs device
- Builds for correct architecture(s)
- Handles Debug/Release configurations
- Creates universal binaries when needed

### `build_ios.sh`

Manual build for all platforms:
```bash
./build_ios.sh              # Release build
./build_ios.sh debug        # Debug build
```

Useful for:
- Testing all targets
- Creating XCFrameworks
- CI/CD pipelines

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

Bevy logs are visible in Xcode console. Configure in your app:
```rust
.set(LogPlugin {
    level: Level::INFO,
    filter: "wgpu=error,bevy_render=info".to_string(),
    ..Default::default()
})
```

## Troubleshooting

See [XCODE_SETUP.md](./XCODE_SETUP.md#troubleshooting) for common issues and solutions.

---

## Android Example

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

# Install cargo-ndk
cargo install cargo-ndk

# Set up Android NDK
export ANDROID_NDK_ROOT=$HOME/Android/Sdk/ndk/26.1.10909125
```

### 2. Build and Run

```bash
cd bevy-embedded-android
../build_android.sh release
```

Then open the project in Android Studio and run it.

### 3. Full Documentation

See [bevy-embedded-android/README.md](./bevy-embedded-android/README.md) for complete setup instructions, API reference, and troubleshooting.

## License

This example is dual-licensed under MIT or Apache 2.0, matching Bevy's license.
