# pydocstring task runner — run `just` or `just --list` to see recipes.

py_dir := "bindings/python"

# Show all available recipes
default:
    @just --list

# ---- Rust --------------------------------------------------------------------

# Format Rust sources
fmt:
    cargo +nightly fmt --all

# Check formatting without modifying files
fmt-check:
    cargo +nightly fmt --all -- --check

# Lint with clippy (warnings are errors)
lint:
    cargo clippy --all-targets -- -D warnings

# Build the crate
build:
    cargo build

# Run the Rust test suite
test:
    cargo test

# ---- Python bindings (uv + maturin) ------------------------------------------

# Create/refresh the Python dev environment
py-sync:
    cd {{py_dir}} && uv sync

# Build & install the native extension into the venv
py-dev: py-sync
    cd {{py_dir}} && uv run maturin develop --uv

# Run the Python test suite (rebuilds the extension first)
py-test: py-dev
    cd {{py_dir}} && uv run pytest tests/ -v

# Format Python sources (ruff)
py-fmt:
    cd {{py_dir}} && uv run ruff format

# Check Python formatting + lint without modifying files (ruff)
py-lint:
    cd {{py_dir}} && uv run ruff format --check
    cd {{py_dir}} && uv run ruff check

# Auto-fix Python formatting and lint issues (ruff)
py-fix:
    cd {{py_dir}} && uv run ruff format
    cd {{py_dir}} && uv run ruff check --fix

# Type-check Python sources (ty); needs the native extension built
py-typecheck: py-dev
    cd {{py_dir}} && uv run ty check

# All Python checks CI runs: lint, type-check, tests
py-ci: py-lint py-typecheck py-test

# ---- Aggregates --------------------------------------------------------------

# Everything CI runs: Rust format check, lint, tests + all Python checks
ci: fmt-check lint test py-ci

# Checks run by the pre-commit hook (auto-formats Rust & Python, then lints)
pre-commit: fmt lint py-fix
