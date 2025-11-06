#!/usr/bin/env bash

# Bevy Embedded Android Build Script
# Automatically builds the Rust library for Android architectures

set -e

echo "🦀 Building Bevy Embedded for Android"

# ============================================================================
# Configuration - User customizable
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BEVY_RUST_PROJECT="$SCRIPT_DIR"

LIBRARY_NAME="bevy_mobile_embedded_example"
ANDROID_PROJECT_DIR="${SCRIPT_DIR}/bevy-embedded-android"

echo "📂 Rust project: ${BEVY_RUST_PROJECT}"
echo "📂 Android project: ${ANDROID_PROJECT_DIR}"

# Verify the Cargo.toml exists
if [ ! -f "${BEVY_RUST_PROJECT}/Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found at ${BEVY_RUST_PROJECT}/Cargo.toml"
    exit 1
fi

# ============================================================================
# Environment Setup
# ============================================================================

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Use project's target directory
export CARGO_TARGET_DIR="${BEVY_RUST_PROJECT}/../../target"

# ============================================================================
# Determine Build Configuration
# ============================================================================

CARGO_PROFILE="${1:-release}"
PROFILE_DIR="$CARGO_PROFILE"
CARGO_FLAGS=""

if [ "$CARGO_PROFILE" = "release" ]; then
    CARGO_FLAGS="--release"
fi

echo "📋 Profile: ${CARGO_PROFILE}"

# ============================================================================
# Android targets to build
# ============================================================================

TARGETS=(
    "aarch64-linux-android"    # ARM64 (most modern devices)
    "armv7-linux-androideabi"  # ARMv7 (older devices)
    "x86_64-linux-android"     # x86_64 (emulator)
)

echo "🎯 Targets: ${TARGETS[*]}"

# Install all targets
echo "📥 Ensuring targets are installed..."
for TARGET in "${TARGETS[@]}"; do
    rustup target add "$TARGET" 2>/dev/null || true
done

# ============================================================================
# Configure NDK
# ============================================================================

if [ -z "${ANDROID_NDK_ROOT}" ]; then
    # Try to find NDK in common locations
    if [ -d "$HOME/Android/Sdk/ndk" ]; then
        # Find the latest NDK version
        ANDROID_NDK_ROOT=$(ls -d $HOME/Android/Sdk/ndk/* | sort -V | tail -1)
        echo "📍 Found NDK at: ${ANDROID_NDK_ROOT}"
    elif [ -d "$ANDROID_HOME/ndk" ]; then
        ANDROID_NDK_ROOT=$(ls -d $ANDROID_HOME/ndk/* | sort -V | tail -1)
        echo "📍 Found NDK at: ${ANDROID_NDK_ROOT}"
    else
        echo "❌ Error: ANDROID_NDK_ROOT not set and could not find NDK"
        echo "   Please set ANDROID_NDK_ROOT environment variable or install Android NDK"
        echo "   Example: export ANDROID_NDK_ROOT=\$HOME/Android/Sdk/ndk/26.1.10909125"
        exit 1
    fi
else
    # If ANDROID_NDK_ROOT is set but doesn't contain toolchains, it might be pointing
    # to the ndk directory instead of a specific version. Try to find the version.
    if [ ! -d "${ANDROID_NDK_ROOT}/toolchains" ]; then
        if [ -d "${ANDROID_NDK_ROOT}" ]; then
            # Find the latest NDK version in this directory
            NDK_VERSION=$(ls -d ${ANDROID_NDK_ROOT}/* 2>/dev/null | sort -V | tail -1)
            if [ -n "$NDK_VERSION" ] && [ -d "$NDK_VERSION/toolchains" ]; then
                ANDROID_NDK_ROOT="$NDK_VERSION"
                echo "📍 Using NDK version: ${ANDROID_NDK_ROOT}"
            fi
        fi
    fi
fi

export ANDROID_NDK_ROOT

# ============================================================================
# Setup cargo-ndk if not installed
# ============================================================================

if ! command -v cargo-ndk &> /dev/null; then
    echo "📦 Installing cargo-ndk..."
    cargo install cargo-ndk
fi

# ============================================================================
# Build for each architecture
# ============================================================================

echo ""
echo "🔨 Building libraries..."

for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Building for $TARGET..."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    cd "$BEVY_RUST_PROJECT"

    # Determine Android platform from target
    case "$TARGET" in
        "aarch64-linux-android")
            ANDROID_PLATFORM="29"
            ANDROID_ABI="arm64-v8a"
            ;;
        "armv7-linux-androideabi")
            ANDROID_PLATFORM="29"
            ANDROID_ABI="armeabi-v7a"
            ;;
        "x86_64-linux-android")
            ANDROID_PLATFORM="29"
            ANDROID_ABI="x86_64"
            ;;
        *)
            echo "❌ Unsupported target: $TARGET"
            exit 1
            ;;
    esac

    # Build using cargo-ndk
    cargo ndk --target $TARGET --platform $ANDROID_PLATFORM build $CARGO_FLAGS --lib

    # Find the built library
    LIB_PATH="$CARGO_TARGET_DIR/$TARGET/$PROFILE_DIR/lib${LIBRARY_NAME}.so"

    if [ ! -f "$LIB_PATH" ]; then
        echo "❌ Library not found at: $LIB_PATH"
        exit 1
    fi

    echo "✅ Built: $LIB_PATH"

    # Copy to Android project's jniLibs
    JNI_LIBS_DIR="${ANDROID_PROJECT_DIR}/app/src/main/jniLibs/${ANDROID_ABI}"
    mkdir -p "$JNI_LIBS_DIR"
    cp "$LIB_PATH" "$JNI_LIBS_DIR/"

    echo "📦 Copied to: ${JNI_LIBS_DIR}/lib${LIBRARY_NAME}.so"

    # Copy libc++_shared.so from NDK
    # Map Rust target to NDK sysroot triple
    case "$TARGET" in
        "aarch64-linux-android")
            NDK_TRIPLE="aarch64-linux-android"
            ;;
        "armv7-linux-androideabi")
            NDK_TRIPLE="arm-linux-androideabi"
            ;;
        "x86_64-linux-android")
            NDK_TRIPLE="x86_64-linux-android"
            ;;
        *)
            NDK_TRIPLE="$TARGET"
            ;;
    esac

    # Detect host platform
    case "$(uname -s)" in
        Linux*)  NDK_HOST="linux-x86_64" ;;
        Darwin*) NDK_HOST="darwin-x86_64" ;;
        *) NDK_HOST="linux-x86_64" ;;
    esac

    LIBCXX_PATH="${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/${NDK_HOST}/sysroot/usr/lib/${NDK_TRIPLE}/libc++_shared.so"
    if [ -f "$LIBCXX_PATH" ]; then
        cp "$LIBCXX_PATH" "$JNI_LIBS_DIR/"
        echo "📦 Copied libc++_shared.so"
    else
        echo "⚠️  Warning: libc++_shared.so not found at: $LIBCXX_PATH"
    fi

    # Show library info
    SIZE=$(du -h "$LIB_PATH" | cut -f1)
    echo "   Size: $SIZE"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🎉 All builds complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📦 Libraries copied to Android project:"
for TARGET in "${TARGETS[@]}"; do
    case "$TARGET" in
        "aarch64-linux-android") ABI="arm64-v8a" ;;
        "armv7-linux-androideabi") ABI="armeabi-v7a" ;;
        "x86_64-linux-android") ABI="x86_64" ;;
    esac
    echo "  - app/src/main/jniLibs/${ABI}/lib${LIBRARY_NAME}.so"
done

echo ""
echo "💡 Next steps:"
echo "   1. Open ${ANDROID_PROJECT_DIR} in Android Studio"
echo "   2. Build and run the app"
echo ""
