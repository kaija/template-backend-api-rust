# Rust API Microservice Template

A production-ready Rust API microservice template built with modern libraries and best practices.

## 🚀 Features

- **High-Performance**: Built on Tokio async runtime with Axum web framework
- **Database Integration**: SQLx with compile-time checked queries and migrations
- **Configuration Management**: Hierarchical configuration with file, environment, and Vault support
- **Observability**: Structured logging with tracing and Sentry error monitoring
- **Security**: Authentication middleware, rate limiting, and input validation
- **Testing**: Comprehensive test suite with unit and integration tests
- **Container Ready**: Optimized Docker images with security best practices
- **Development Tools**: Hot reloading, linting, formatting, and documentation

## 🛠️ Technology Stack

- **Runtime**: [Tokio](https://tokio.rs/) - Async runtime
- **Web Framework**: [Axum](https://github.com/tokio-rs/axum) - Modern web framework
- **Database**: [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- **Serialization**: [Serde](https://serde.rs/) - JSON/YAML/TOML support
- **Configuration**: [Config-rs](https://github.com/mehcode/config-rs) - Layered configuration
- **Logging**: [Tracing](https://tracing.rs/) - Structured logging and spans
- **HTTP Client**: [Reqwest](https://github.com/seanmonstar/reqwest) - Async HTTP client
- **Error Monitoring**: [Sentry](https://sentry.io/) - Error tracking and performance monitoring

## 📋 Prerequisites

- Rust 1.70+ 
- PostgreSQL 12+
- Docker (optional)

## 🏃 Quick Start

1. **Clone and setup**:
   ```bash
   git clone <repository-url>
   cd rust-api-microservice-template
   cp .env.example .env
   ```

2. **Install development tools**:
   ```bash
   make install-dev
   ```

3. **Setup database**:
   ```bash
   make db-create
   make db-migrate
   ```

4. **Run the application**:
   ```bash
   make run
   ```

The API will be available at `http://localhost:8080`

## 🔧 Development

### Available Commands

```bash
make help           # Show all available commands
make dev            # Run with hot reloading
make test           # Run tests
make lint           # Run clippy lints
make format         # Format code
make doc            # Generate documentation
```

### Project Structure

```
src/
├── config/         # Configuration management
├── models/         # Domain models and DTOs
├── repository/     # Data access layer
├── services/       # Business logic layer
├── utils/          # Utility functions
├── web/            # Web layer (handlers, middleware, etc.)
└── main.rs         # Application entry point

config/             # Configuration files
tests/              # Integration tests
migrations/         # Database migrations
```

## 📊 API Endpoints

### Health Checks
- `GET /health` - Detailed health information
- `GET /health/live` - Liveness probe
- `GET /health/ready` - Readiness probe

### Users API
- `POST /api/v1/users` - Create user
- `GET /api/v1/users/{id}` - Get user by ID
- `PUT /api/v1/users/{id}` - Update user
- `DELETE /api/v1/users/{id}` - Delete user
- `GET /api/v1/users` - List users (with pagination)

## ⚙️ Configuration

Configuration is loaded from multiple sources in priority order:

1. Command line arguments
2. Environment variables (prefixed with `APP_`)
3. Configuration files (`config/default.yaml`, `config/{environment}.yaml`)
4. HashiCorp Vault (optional)

### Environment Variables

```bash
# Database
APP_DATABASE__URL=postgresql://user:pass@localhost/dbname

# Server
APP_SERVER__HOST=0.0.0.0
APP_SERVER__PORT=8080

# Logging
APP_LOGGING__LEVEL=info

# Sentry
APP_SENTRY__DSN=https://your-dsn@sentry.io/project
```

## 🐳 Docker

### Build and run with Docker:

```bash
make docker-build
make docker-run
```

### Using Docker Compose:

```bash
docker-compose up -d
```

## 🧪 Testing

```bash
# Run all tests
make test

# Run specific test
cargo test test_name

# Run with coverage
cargo tarpaulin --out html
```

## 📈 Monitoring and Observability

- **Structured Logging**: JSON formatted logs with correlation IDs
- **Tracing**: Request tracing with spans for performance monitoring
- **Metrics**: Prometheus-compatible metrics endpoint at `/metrics`
- **Error Monitoring**: Automatic error capture with Sentry
- **Health Checks**: Kubernetes-compatible health endpoints

## 🔒 Security

- Non-root container execution
- Input validation with detailed error messages
- Rate limiting middleware
- CORS configuration
- Secure password hashing with Argon2
- JWT-based authentication (TODO)

## 🚀 Deployment

### Kubernetes

Example deployment manifests are provided in the `k8s/` directory:

```bash
kubectl apply -f k8s/
```

### Environment Variables for Production

```bash
APP_ENVIRONMENT=production
APP_DATABASE__URL=postgresql://...
APP_SENTRY__DSN=https://...
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `make check test lint`
6. Submit a pull request

## 📄 License

This project is licensed under the MIT OR Apache-2.0 license.

## 🆘 Support

- Check the [documentation](./docs/)
- Open an [issue](https://github.com/yourusername/rust-api-microservice-template/issues)
- Review the [troubleshooting guide](./docs/troubleshooting.md)

---

Built with ❤️ using Rust and modern best practices.