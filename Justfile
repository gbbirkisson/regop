# Show this help
default:
  @just --list

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

# Little test runs
run:
  cargo run -- \
    -r 'version = "(?<major>[^\.])\.(?<minor>[^\.])\.(?<patch>[^\.])"' \
    -o "<major>:inc" \
    -o "<minor>:inc:2" \
    -o "<patch>:inc:10" \
    Cargo.toml

  cargo run -- \
    -l \
    -r '^version = "(?<major>[^\.])\.(?<minor>[^\.])\.(?<patch>[^\.])"$' \
    -o "<major>:inc" \
    -o "<minor>:inc:2" \
    -o "<patch>:inc:10" \
    Cargo.toml

# Install locally
install:
  cargo install --path .

# Install cargo dist
install-dist:
  cargo install --git https://github.com/astral-sh/cargo-dist.git --tag v0.28.4 cargo-dist
