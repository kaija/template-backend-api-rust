#!/bin/bash

# Comprehensive build script for the Rust API microservice
# Supports both local and Docker builds

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to detect and export build environment variables
setup_build_env() {
    print_status "Setting up build environment variables..."
    
    # Get Rust version
    if command -v rustc >/dev/null 2>&1; then
        export RUSTC_VERSION=$(rustc --version | cut -d' ' -f2)
        print_success "RUSTC_VERSION: $RUSTC_VERSION"
    else
        export RUSTC_VERSION="unknown"
        print_warning "rustc not found, using default version: $RUSTC_VERSION"
    fi

    # Get target architecture
    if command -v rustc >/dev/null 2>&1; then
        export TARGET=$(rustc -vV | grep host | cut -d' ' -f2)
        print_success "TARGET: $TARGET"
    else
        export TARGET="unknown"
        print_warning "Unable to detect target, using default: $TARGET"
    fi

    # Generate build timestamp
    export BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    print_success "BUILD_TIMESTAMP: $BUILD_TIMESTAMP"

    # Export to environment file
    cat > .env.build << EOF
RUSTC_VERSION=$RUSTC_VERSION
TARGET=$TARGET
BUILD_TIMESTAMP=$BUILD_TIMESTAMP
EOF

    print_success "Build environment variables exported to .env.build"
}

# Function to build locally
build_local() {
    print_status "Starting local build..."
    
    setup_build_env
    
    print_status "Building Rust project..."
    cargo build --release
    
    print_success "Local build completed successfully!"
    echo ""
    print_status "You can now run the application with:"
    echo "   cargo run"
    echo "   or"
    echo "   ./target/release/rust-api"
}

# Function to build with Docker
build_docker() {
    print_status "Starting Docker build..."
    
    setup_build_env
    
    print_status "Building Docker image with build arguments..."
    docker-compose build \
        --build-arg RUSTC_VERSION="$RUSTC_VERSION" \
        --build-arg TARGET="x86_64-unknown-linux-gnu" \
        --build-arg BUILD_TIMESTAMP="$BUILD_TIMESTAMP" \
        api
    
    print_success "Docker build completed successfully!"
    echo ""
    print_status "You can now run the application with:"
    echo "   docker-compose up api"
}

# Function to run tests
run_tests() {
    print_status "Running tests..."
    
    setup_build_env
    
    cargo test
    
    print_success "Tests completed successfully!"
}

# Function to clean build artifacts
clean() {
    print_status "Cleaning build artifacts..."
    
    cargo clean
    
    if [ -f ".env.build" ]; then
        rm .env.build
        print_status "Removed .env.build"
    fi
    
    print_success "Clean completed successfully!"
}

# Function to show help
show_help() {
    echo "Rust API Microservice Build Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  local     Build the application locally (default)"
    echo "  docker    Build the application with Docker"
    echo "  test      Run tests"
    echo "  clean     Clean build artifacts"
    echo "  env       Show current build environment variables"
    echo "  help      Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0              # Build locally"
    echo "  $0 local        # Build locally"
    echo "  $0 docker       # Build with Docker"
    echo "  $0 test         # Run tests"
    echo "  $0 clean        # Clean build artifacts"
}

# Function to show environment variables
show_env() {
    setup_build_env
    
    echo ""
    print_status "Current build environment:"
    echo "   RUSTC_VERSION=$RUSTC_VERSION"
    echo "   TARGET=$TARGET"
    echo "   BUILD_TIMESTAMP=$BUILD_TIMESTAMP"
    echo ""
}

# Main script logic
case "${1:-local}" in
    "local")
        build_local
        ;;
    "docker")
        build_docker
        ;;
    "test")
        run_tests
        ;;
    "clean")
        clean
        ;;
    "env")
        show_env
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        print_error "Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac