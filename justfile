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

# Build the docs, failing on a broken intra-doc link
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Every Rust public item must be exposed in Python, or excused in writing.
#
# Depends on py-dev: the check introspects the *installed* extension module, so
# without a rebuild it reads the previous one. Proven, not theorised — deleting
# `replace_in` from the bindings (the very method #115 was filed about) left this
# check green. The Rust half regenerates its own rustdoc JSON for the same reason.
api-parity: py-dev
    python3 scripts/api_parity.py

# Build the crate
build:
    cargo build

# Run the Rust test suite
test:
    cargo test

# Rust line-coverage summary (runs the full test suite incl. corpus/law tests).
# Report-only: never gates CI and enforces no thresholds.
coverage:
    cargo llvm-cov --all-targets --summary-only

# Export lcov from the profile data of the last `just coverage` run
# (cargo llvm-cov report reuses profdata; no second test run).
coverage-lcov:
    cargo llvm-cov report --lcov --output-path lcov.info

# Rust coverage as a browsable HTML report (written to target/llvm-cov/html)
coverage-html:
    cargo llvm-cov --all-targets --html

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

# Run only the sphinx.ext.napoleon differential parity suite (needs sphinx,
# which is a dev dependency; folded into py-test/py-ci automatically)
py-napoleon: py-dev
    cd {{py_dir}} && uv run pytest tests/test_napoleon_differential.py -v

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

# Python test coverage (report-only, no thresholds). Only the pure-Python
# files (__init__.py, _visitor.py) are measured: the extension itself is
# compiled Rust, which pytest-cov cannot see. The bindings' Rust code is a
# known coverage gap — `just coverage` only covers the root crate's tests,
# while the bindings crate is exercised via pytest outside llvm-cov. Revisit
# via cargo-llvm-cov --include-ffi / maturin integration if it ever matters.
py-coverage: py-dev
    cd {{py_dir}} && uv run pytest tests/ --cov=pydocstring --cov-report=term-missing

# Type-check Python sources (ty); needs the native extension built
py-typecheck: py-dev
    cd {{py_dir}} && uv run ty check

# All Python checks CI runs: lint, type-check, tests
py-ci: py-lint py-typecheck py-test

# ---- Aggregates --------------------------------------------------------------

# Everything CI runs: Rust format check, lint, tests + all Python checks
ci: fmt-check lint doc test py-ci api-parity

# Checks run by the pre-commit hook (auto-formats Rust & Python, then lints)
pre-commit: fmt lint py-fix
