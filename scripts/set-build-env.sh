#!/bin/bash

# Script to automatically detect and export build environment variables
# This script sets the required environment variables for the Rust build

set -e

echo "🔧 Setting up build environment variables..."

# Get Rust version
if command -v rustc >/dev/null 2>&1; then
    RUSTC_VERSION=$(rustc --version | cut -d' ' -f2)
    echo "✅ RUSTC_VERSION: $RUSTC_VERSION"
    export RUSTC_VERSION
else
    echo "❌ rustc not found, using default version"
    export RUSTC_VERSION="unknown"
fi

# Get target architecture
if command -v rustc >/dev/null 2>&1; then
    TARGET=$(rustc -vV | grep host | cut -d' ' -f2)
    echo "✅ TARGET: $TARGET"
    export TARGET
else
    echo "❌ Unable to detect target, using default"
    export TARGET="unknown"
fi

# Generate build timestamp
BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "✅ BUILD_TIMESTAMP: $BUILD_TIMESTAMP"
export BUILD_TIMESTAMP

# Export all variables to a file for Docker builds
cat > /tmp/build-env.sh << EOF
export RUSTC_VERSION="$RUSTC_VERSION"
export TARGET="$TARGET"
export BUILD_TIMESTAMP="$BUILD_TIMESTAMP"
EOF

echo "🎉 Build environment variables set successfully!"
echo "📝 Variables exported to /tmp/build-env.sh"

# If running in Docker build context, also export to /app/build-env.sh
if [ -d "/app" ]; then
    cp /tmp/build-env.sh /app/build-env.sh
    echo "📝 Variables also exported to /app/build-env.sh"
fi

# Print all variables for verification
echo ""
echo "📋 Current build environment:"
echo "   RUSTC_VERSION=$RUSTC_VERSION"
echo "   TARGET=$TARGET"
echo "   BUILD_TIMESTAMP=$BUILD_TIMESTAMP"
echo ""