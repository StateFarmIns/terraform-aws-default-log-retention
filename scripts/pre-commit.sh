#!/bin/bash
# Just a little helper script to run "all the things" possible before committing/pushing. Convenience.

./scripts/test.sh || exit 1

cargo clippy --fix --allow-dirty --allow-staged || exit 1
cargo fmt || exit 1
cargo audit -D warnings || exit 1

# ./scripts/coverage.sh || exit 1
# git add reports/*

# ./scripts/build-release.sh || exit 1
# # git add dist/* # Do not add these files; semantic release during pipeline will do so.

# echo "Coverage reports lint fixes are staged. Please commit and push."
