#!/bin/bash

# TODO: This script doesn't work inside State Farm; check it out on a non-corporate laptop.

docker run --security-opt seccomp=unconfined -v "$(pwd):/volume" xd009642/tarpaulin sh -c "cargo tarpaulin --skip-clean -- --test-threads=1"