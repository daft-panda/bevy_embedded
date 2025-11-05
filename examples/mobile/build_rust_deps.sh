#!/usr/bin/env bash

# Bevy Embedded iOS Build Script
# Automatically builds the Rust library for the correct architecture and configuration

set -e

echo "🦀 Building Bevy Embedded for iOS"

# ============================================================================
# Configuration - User customizable
# ============================================================================

# Path to your Bevy project root (containing Cargo.toml)
# This can be set as a User-Defined setting in Xcode build settings
# Default: one directory up from the script location (assumes script is in project root)
if [ -z "${BEVY_RUST_PROJECT_PATH}" ]; then
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    BEVY_RUST_PROJECT="$SCRIPT_DIR"
else
    BEVY_RUST_PROJECT="${BEVY_RUST_PROJECT_PATH}"
fi

LIBRARY_NAME="bevy_mobile_embedded_example"

echo "📂 Rust project: ${BEVY_RUST_PROJECT}"

# Verify the Cargo.toml exists
if [ ! -f "${BEVY_RUST_PROJECT}/Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found at ${BEVY_RUST_PROJECT}/Cargo.toml"
    echo "   Please set BEVY_RUST_PROJECT_PATH in Xcode Build Settings"
    exit 1
fi

# ============================================================================
# Environment Setup
# ============================================================================

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Add homebrew for any tools that might be needed
export PATH="$PATH:/opt/homebrew/bin:/usr/local/bin"

# Use project's target directory (outside Xcode sandbox restrictions)
# This avoids Xcode sandbox permission issues
export CARGO_TARGET_DIR="${BEVY_RUST_PROJECT}/../../target"

# Fix Xcode toolchain path issue that causes 'ld: library System not found'
# See: https://github.com/rust-lang/rust/issues/80817
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin:$PATH"

# ============================================================================
# Determine Build Configuration
# ============================================================================

CARGO_PROFILE="dev"
PROFILE_DIR="debug"
CARGO_FLAGS=""

if [ "${CONFIGURATION}" = "Release" ]; then
    CARGO_PROFILE="release"
    PROFILE_DIR="release"
    CARGO_FLAGS="--release"
fi

echo "📋 Configuration: ${CONFIGURATION}"
echo "📦 Profile: ${CARGO_PROFILE}"
echo "🏗️  Platform: ${PLATFORM_NAME}"
echo "🎯 Architectures: ${ARCHS}"

# ============================================================================
# Determine if this is a simulator build
# ============================================================================

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
    IS_SIMULATOR=1
fi

# ============================================================================
# Build for each architecture
# ============================================================================

BUILT_LIBS=()

for ARCH in $ARCHS; do
    case "$ARCH" in
        x86_64)
            if [ $IS_SIMULATOR -eq 0 ]; then
                echo "❌ Building for x86_64 but not a simulator build" >&2
                exit 1
            fi
            RUST_TARGET="x86_64-apple-ios"
            ;;

        arm64)
            if [ $IS_SIMULATOR -eq 0 ]; then
                # Real device
                RUST_TARGET="aarch64-apple-ios"
            else
                # Simulator on Apple Silicon
                RUST_TARGET="aarch64-apple-ios-sim"
            fi
            ;;

        *)
            echo "❌ Unsupported architecture: $ARCH" >&2
            exit 1
            ;;
    esac

    echo "🔨 Building for $RUST_TARGET..."

    # Ensure the target is installed
    rustup target add "$RUST_TARGET" 2>/dev/null || true

    # Build the library
    cd "$BEVY_RUST_PROJECT"
    cargo rustc $CARGO_FLAGS --target "$RUST_TARGET" --lib -- \
        -C link-arg=-Wl,-undefined,dynamic_lookup

    # Locate the built dylib
    DYLIB_PATH="$CARGO_TARGET_DIR/$RUST_TARGET/$PROFILE_DIR/lib${LIBRARY_NAME}.dylib"

    if [ ! -f "$DYLIB_PATH" ]; then
        echo "❌ Library not found at: $DYLIB_PATH" >&2
        exit 1
    fi

    # Set the install_name for the dylib
    install_name_tool -id "@rpath/lib${LIBRARY_NAME}.dylib" "$DYLIB_PATH"

    BUILT_LIBS+=("$DYLIB_PATH")
    echo "✅ Built: $DYLIB_PATH"
done

# ============================================================================
# Create universal library and copy to app bundle
# ============================================================================

# First, create the universal library in build products
OUTPUT_DIR="${BUILT_PRODUCTS_DIR}"
mkdir -p "$OUTPUT_DIR"

if [ ${#BUILT_LIBS[@]} -gt 1 ]; then
    echo "🔗 Creating universal dylib with lipo..."
    UNIVERSAL_LIB="$OUTPUT_DIR/lib${LIBRARY_NAME}.dylib"
    lipo -create "${BUILT_LIBS[@]}" -output "$UNIVERSAL_LIB"

    # Set install_name on universal library
    install_name_tool -id "@rpath/lib${LIBRARY_NAME}.dylib" "$UNIVERSAL_LIB"
    echo "✅ Universal library: $UNIVERSAL_LIB"
else
    # Single architecture
    UNIVERSAL_LIB="$OUTPUT_DIR/lib${LIBRARY_NAME}.dylib"
    cp "${BUILT_LIBS[0]}" "$UNIVERSAL_LIB"
    echo "✅ Copied library: $UNIVERSAL_LIB"
fi

# ============================================================================
# Create symlink in project directory for Xcode
# ============================================================================

# Check if sandboxing is disabled (required for creating symlinks)
if [ "${ENABLE_USER_SCRIPT_SANDBOXING}" = "YES" ]; then
    echo "⚠️  WARNING: User script sandboxing is enabled!"
    echo "   Symlink creation will be skipped."
    echo "   To enable symlink creation, set ENABLE_USER_SCRIPT_SANDBOXING=NO"
    echo "   in your Xcode project's Build Settings."
    echo ""
    echo "📍 Library output: $UNIVERSAL_LIB"
else
    echo "🔗 Creating symlink in project directory..."

    # Use TARGET_BUILD_DIR to find the target directory, then go up to project
    # Or use PROJECT_DIR if available
    if [ -n "${PROJECT_DIR}" ]; then
        SYMLINK_DIR="${PROJECT_DIR}/${TARGET_NAME}"
        mkdir -p "${SYMLINK_DIR}"
        SYMLINK_PATH="${SYMLINK_DIR}/lib${LIBRARY_NAME}.dylib"
    else
        echo "❌ PROJECT_DIR not set, cannot create symlink"
        exit 1
    fi

    # Remove old symlink if it exists
    if [ -L "$SYMLINK_PATH" ] || [ -f "$SYMLINK_PATH" ]; then
        rm -f "$SYMLINK_PATH"
    fi

    # Create symlink pointing to the universal library
    ln -sf "$UNIVERSAL_LIB" "$SYMLINK_PATH"

    echo "✅ Symlink created: $SYMLINK_PATH -> $UNIVERSAL_LIB"
    echo ""
    echo "💡 Add $SYMLINK_PATH to your Xcode target's"
    echo "   'Frameworks, Libraries, and Embedded Content'"
    echo "   The symlink automatically updates for device/simulator builds"
fi

echo "🎉 Bevy Embedded build complete!"
