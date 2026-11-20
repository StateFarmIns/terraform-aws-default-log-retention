#!/bin/bash
# Runs code coverage natively on Linux or macOS (no Docker needed).
# Settings live in .tarpaulin.toml. Reports are written to reports/.
set -euo pipefail

if ! cargo tarpaulin --version >/dev/null 2>&1; then
  echo "cargo-tarpaulin not found; installing..."
  cargo install cargo-tarpaulin
fi

cargo tarpaulin --skip-clean "$@"

echo "HTML report: reports/tarpaulin-report.html"
