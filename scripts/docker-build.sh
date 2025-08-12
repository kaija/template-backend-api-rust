#!/bin/bash

# Docker build script that sets up environment variables for containerized builds

set -e

echo "🐳 Starting Docker build with environment setup..."

# Set build arguments for Docker
export RUSTC_VERSION=$(rustc --version 2>/dev/null | cut -d' ' -f2 || echo "1.80.0")
export TARGET=$(rustc -vV 2>/dev/null | grep host | cut -d' ' -f2 || echo "x86_64-unknown-linux-gnu")
export BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "📋 Build environment:"
echo "   RUSTC_VERSION=$RUSTC_VERSION"
echo "   TARGET=$TARGET"
echo "   BUILD_TIMESTAMP=$BUILD_TIMESTAMP"

# Build with Docker Compose
docker-compose build \
    --build-arg RUSTC_VERSION="$RUSTC_VERSION" \
    --build-arg TARGET="$TARGET" \
    --build-arg BUILD_TIMESTAMP="$BUILD_TIMESTAMP" \
    api

echo "✅ Docker build completed successfully!"