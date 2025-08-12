#!/bin/bash

# Local build script that sets up environment variables and builds the project

set -e

echo "🚀 Starting local build..."

# Source the environment setup script
source scripts/set-build-env.sh

# Build the project
echo "🔨 Building Rust project..."
cargo build --release

echo "✅ Build completed successfully!"
echo ""
echo "🎯 You can now run the application with:"
echo "   cargo run"
echo "   or"
echo "   ./target/release/rust-api"