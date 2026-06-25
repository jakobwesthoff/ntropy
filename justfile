# ntropy development tasks

# List available recipes
default:
    @just --list

# Run the test suite
test:
    cargo test

# Lint with clippy, denying all warnings
clippy:
    cargo clippy --all-targets -- -D warnings

# Format the codebase
fmt:
    cargo fmt

# Verify formatting, lints and tests (CI gate)
check: clippy test
    cargo fmt --check

# Measure test coverage
coverage:
    cargo llvm-cov
