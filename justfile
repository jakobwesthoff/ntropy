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

# Install the frontend's dependencies with Bun (site/)
site-install:
    cd site && bun install --frozen-lockfile

# Build the browser-side files into src/site/dist/ (commit the result)
site-build: site-install
    cd site && bun run build

# Type-check, lint, format-check, and test the frontend
site-test: site-install
    cd site && bun run typecheck && bunx biome ci && bun run test

# Verify the committed src/site/dist/ matches the frontend sources (CI gate)
site-check: site-build site-test
    git diff --exit-code -- src/site/dist

# Run the tests that need the real typst binary: the kitchen-sink fixture,
# whose pdf/png/typ artifacts land under target/verify-render/ for optical
# inspection, and the note-link annotation check
verify-render:
    cargo test --test cli -- --ignored --nocapture

# Measure test coverage
coverage:
    cargo llvm-cov

# Benchmark access and query patterns against a generated vault (needs hyperfine)
bench *ARGS:
    ./scripts/benchmark.sh {{ARGS}}

# Export the documentation vault (docs/website) as the project website into ./dist
website:
    cargo run --quiet -- --vault docs/website -n site -o dist --force --strict
