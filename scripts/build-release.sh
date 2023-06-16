#!/bin/bash

cargo lambda build --profile release-lambda --arm64 || exit 1

mkdir -p dist/log_retention_setter
cp target/lambda/terraform-aws-default-log-retention/bootstrap dist/log_retention_setter/

mkdir -p dist/global_retention_setter
cp target/lambda/global_retention_setter/bootstrap dist/global_retention_setter/
