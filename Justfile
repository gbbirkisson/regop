# Show this help
default:
  just --list

# Build release
build:
  cargo build --release

# Run clippy linter
lint-clippy:
  cargo clippy -- --no-deps -D warnings

# Run fmt linter
lint-fmt:
  cargo fmt -- --check

# Run tests
test:
  cargo test

# Run CI pipeline
ci: lint-fmt lint-clippy test

# Recreate release.yml workflow
dist:
  dist init -y

# Install locally
install:
  cargo install --path .
