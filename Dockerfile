# Build stage
FROM rust:1.86-slim as builder

# Accept build arguments
ARG RUSTC_VERSION
ARG TARGET
ARG BUILD_TIMESTAMP

# Set environment variables from build args
ENV RUSTC_VERSION=${RUSTC_VERSION}
ENV TARGET=${TARGET}
ENV BUILD_TIMESTAMP=${BUILD_TIMESTAMP}

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy build scripts and source code
COPY scripts ./scripts
COPY build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY config ./config

# Set build environment variables and build the application
RUN chmod +x scripts/set-build-env.sh && \
    ./scripts/set-build-env.sh && \
    . /tmp/build-env.sh && \
    cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false -m -d /app appuser

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rust-api /app/rust-api

# Copy configuration files
COPY --from=builder /app/config /app/config
COPY --from=builder /app/migrations /app/migrations

# Change ownership to app user
RUN chown -R appuser:appuser /app

# Switch to app user
USER appuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health/live || exit 1

# Run the application
CMD ["./rust-api"]
