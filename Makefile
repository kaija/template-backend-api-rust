# Makefile for Rust API Microservice Template

.PHONY: all check test build run clean format lint doc help

# Default target
all: check test build

# Check code without building
check:
	@echo "🔍 Checking code..."
	cargo check --all-targets --all-features

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test --all-features

# Build the project
build:
	@echo "🔨 Building project..."
	cargo build --release

# Run the application
run:
	@echo "🚀 Running application..."
	cargo run

# Run in development mode with auto-reload
dev:
	@echo "🔄 Running in development mode..."
	cargo watch -x run

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# Format code
format:
	@echo "🎨 Formatting code..."
	cargo fmt --all

# Check formatting
format-check:
	@echo "🎨 Checking code formatting..."
	cargo fmt --all -- --check

# Run clippy lints
lint:
	@echo "📎 Running clippy lints..."
	cargo clippy --all-targets --all-features -- -D warnings

# Generate documentation
doc:
	@echo "📚 Generating documentation..."
	cargo doc --all-features --no-deps --open

# Run security audit
audit:
	@echo "🔒 Running security audit..."
	cargo audit

# Install development dependencies
install-dev:
	@echo "📦 Installing development dependencies..."
	cargo install cargo-watch cargo-audit sqlx-cli

# Database operations
db-create:
	@echo "🗄️ Creating database..."
	createdb rust_api_template

db-drop:
	@echo "🗑️ Dropping database..."
	dropdb rust_api_template

db-migrate:
	@echo "🔄 Running database migrations..."
	sqlx migrate run

db-reset: db-drop db-create db-migrate
	@echo "♻️ Database reset complete"

# Docker operations
docker-build:
	@echo "🐳 Building Docker image..."
	docker build -t rust-api-template .

docker-run:
	@echo "🐳 Running Docker container..."
	docker run -p 8080:8080 rust-api-template

# Help
help:
	@echo "Available targets:"
	@echo "  all          - Run check, test, and build"
	@echo "  check        - Check code without building"
	@echo "  test         - Run tests"
	@echo "  build        - Build the project"
	@echo "  run          - Run the application"
	@echo "  dev          - Run in development mode with auto-reload"
	@echo "  clean        - Clean build artifacts"
	@echo "  format       - Format code"
	@echo "  format-check - Check code formatting"
	@echo "  lint         - Run clippy lints"
	@echo "  doc          - Generate documentation"
	@echo "  audit        - Run security audit"
	@echo "  install-dev  - Install development dependencies"
	@echo "  db-create    - Create database"
	@echo "  db-drop      - Drop database"
	@echo "  db-migrate   - Run database migrations"
	@echo "  db-reset     - Reset database (drop, create, migrate)"
	@echo "  docker-build - Build Docker image"
	@echo "  docker-run   - Run Docker container"
	@echo "  help         - Show this help message"