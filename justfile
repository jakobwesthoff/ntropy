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

# Render the kitchen-sink fixture with the real typst binary and drop the
# pdf/png/typ artifacts under target/verify-render/ for optical inspection
verify-render:
    cargo test --test cli render_kitchen_sink_compiles_with_real_typst -- --ignored --nocapture

# Measure test coverage
coverage:
    cargo llvm-cov

# Benchmark access and query patterns against a generated vault (needs hyperfine)
bench *ARGS:
    ./scripts/benchmark.sh {{ARGS}}
