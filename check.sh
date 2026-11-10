#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo "==> rustfmt"
(
  cd engine
  cargo fmt --check
)

echo "==> clippy"
(
  cd engine
  cargo clippy --workspace --all-targets -- -D warnings
)

echo "==> cargo test"
(
  cd engine
  cargo test --workspace
)

echo "==> TypeScript bindings"
(
  cd engine
  cargo test -p seatlock-core --features typescript export_typescript_bindings -- --exact
)

echo "==> wasm build"
(
  cd engine
  cargo build --target wasm32-unknown-unknown --release -p seatlock-wasm -p seatlock-rt
)
mkdir -p web/public
cp engine/target/wasm32-unknown-unknown/release/seatlock_wasm.wasm web/public/
cp engine/target/wasm32-unknown-unknown/release/seatlock_rt.wasm web/public/
mkdir -p web/public/examples
cp designs/examples/*.seatlock.json web/public/examples/

echo "==> web checks"
(
  cd web
  if [[ ! -d node_modules ]]; then
    npm ci
  fi
  npm run typecheck
  npm run test
  npm run build
  npx playwright install chromium
  npm run test:e2e
)

echo "All checks passed."
