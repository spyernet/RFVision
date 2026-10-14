#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$ROOT"

cargo build --release --offline \
    -p rfv-wizard \
    -p rfv-service \
    -p rfv-adapter-rtw88

printf 'Built wizard and userspace services in %s/target/release\n' "$ROOT"
