# Build Scripts

This directory contains scripts to build the Rust API microservice with proper environment variable setup.

## Problem Solved

The Rust code uses compile-time environment variables (`env!()` macro) for build information:
- `RUSTC_VERSION` - Rust compiler version
- `TARGET` - Target architecture 
- `BUILD_TIMESTAMP` - Build timestamp

These variables need to be set during compilation, especially in Docker builds where they might not be available.

## Scripts

### `build.sh` - Main Build Script

The comprehensive build script that handles all build scenarios:

```bash
# Build locally (default)
./scripts/build.sh
./scripts/build.sh local

# Build with Docker
./scripts/build.sh docker

# Run tests
./scripts/build.sh test

# Clean build artifacts
./scripts/build.sh clean

# Show environment variables
./scripts/build.sh env

# Show help
./scripts/build.sh help
```

### `set-build-env.sh` - Environment Setup

Sets up the required build environment variables:

```bash
./scripts/set-build-env.sh
```

This script:
- Detects the Rust compiler version
- Determines the target architecture
- Generates a build timestamp
- Exports variables to `/tmp/build-env.sh`

### `build-local.sh` - Local Build

Simple script for local development builds:

```bash
./scripts/build-local.sh
```

### `docker-build.sh` - Docker Build

Docker-specific build script with build arguments:

```bash
./scripts/docker-build.sh
```

## Docker Integration

The Dockerfile accepts build arguments:

```dockerfile
ARG RUSTC_VERSION
ARG TARGET  
ARG BUILD_TIMESTAMP

ENV RUSTC_VERSION=${RUSTC_VERSION}
ENV TARGET=${TARGET}
ENV BUILD_TIMESTAMP=${BUILD_TIMESTAMP}
```

The docker-compose.yml passes these arguments:

```yaml
api:
  build:
    context: .
    args:
      RUSTC_VERSION: "1.86.0"
      TARGET: "x86_64-unknown-linux-gnu"
      BUILD_TIMESTAMP: "${BUILD_TIMESTAMP:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
```

## Build Process

1. **Environment Detection**: Scripts automatically detect:
   - Rust compiler version (`rustc --version`)
   - Target architecture (`rustc -vV | grep host`)
   - Current timestamp (`date -u`)

2. **Variable Export**: Variables are exported to:
   - Environment for immediate use
   - `.env.build` file for persistence
   - `/tmp/build-env.sh` for sourcing

3. **Build Execution**: 
   - Local: `cargo build --release`
   - Docker: `docker-compose build` with build args

## Usage Examples

### Local Development
```bash
# Quick build
./scripts/build.sh

# Build and run
./scripts/build.sh local
cargo run

# Run tests
./scripts/build.sh test
```

### Docker Development
```bash
# Build Docker image
./scripts/build.sh docker

# Run with Docker Compose
docker-compose up api
```

### CI/CD Integration
```bash
# In CI/CD pipeline
export BUILD_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
export RUSTC_VERSION="1.86.0"
export TARGET="x86_64-unknown-linux-gnu"

./scripts/build.sh docker
```

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RUSTC_VERSION` | Rust compiler version | `1.86.0` |
| `TARGET` | Target architecture | `x86_64-unknown-linux-gnu` |
| `BUILD_TIMESTAMP` | Build timestamp (ISO 8601) | `2025-08-11T15:05:29Z` |

## Troubleshooting

### Missing rustc
If `rustc` is not found:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Docker Build Fails
If Docker build fails with environment variable errors:
```bash
# Ensure build args are passed
docker build \
  --build-arg RUSTC_VERSION="1.86.0" \
  --build-arg TARGET="x86_64-unknown-linux-gnu" \
  --build-arg BUILD_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  .
```

### Permission Denied
If scripts are not executable:
```bash
chmod +x scripts/*.sh
```

## Integration with build.rs

The project also includes a `build.rs` file that automatically sets these variables if they're not provided:

```rust
// build.rs automatically detects and sets:
// - RUSTC_VERSION from `rustc --version`
// - TARGET from `rustc -vV`  
// - BUILD_TIMESTAMP from current time
```

This provides a fallback for cases where the environment variables aren't set externally.