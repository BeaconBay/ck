#!/usr/bin/env bash
set -euo pipefail

target="${1:?missing Linux target triple}"
# An empty OPENSSL_NO_VENDOR also disables vendoring; explicitly allow it for
# the target libraries. Host build dependencies use the runner's libssl-dev.
export OPENSSL_NO_VENDOR=0
exec cargo build --locked --release --target "$target" --package ck-search \
  --features=openssl/vendored
