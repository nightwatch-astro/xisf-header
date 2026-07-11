# xisf-header task runner. Run `just` to list recipes.

# Show the available recipes.
default:
    @just --list

# Compile the crate and all targets.
build:
    cargo build --all-targets

# Run the full test suite (unit, integration, and doc tests).
test:
    cargo test --all-targets
    cargo test --doc

# Lint: clippy (warnings as errors) + rustfmt check.
lint:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo fmt --all --check

# Auto-format the source tree.
fmt:
    cargo fmt --all

# Watch-and-test loop (requires cargo-watch).
dev:
    cargo watch -x "test --all-targets"

# Build the API docs.
doc:
    cargo doc --no-deps --open

# Dry-run the crates.io package to confirm it is publishable.
publish-check:
    cargo publish --dry-run

# Remove build artifacts.
clean:
    cargo clean
