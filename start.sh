#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

require() {
  local name="$1"
  local install_hint="$2"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "Missing: $name"
    echo "Install: $install_hint"
    exit 1
  fi
}

require rustup 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env"'
require cargo 'rustup component add cargo'
require node 'curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt install -y nodejs'
require npm 'sudo apt install -y npm'
require xdg-open 'sudo apt install -y xdg-utils'

NODE_MAJOR="$(node -p 'Number(process.versions.node.split(".")[0])')"
NODE_MINOR="$(node -p 'Number(process.versions.node.split(".")[1])')"
if (( NODE_MAJOR < 22 || (NODE_MAJOR == 22 && NODE_MINOR < 12) )); then
  echo "Node.js 22.12 or newer is required (found $(node --version))."
  echo "Install: curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt install -y nodejs"
  exit 1
fi

rustup show >/dev/null 2>&1 || true
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
  echo "Installing wasm32-unknown-unknown target..."
  rustup target add wasm32-unknown-unknown
fi

echo "Building WebAssembly modules..."
(
  cd engine
  cargo build --target wasm32-unknown-unknown --release -p seatlock-wasm -p seatlock-rt
)
mkdir -p web/public/examples
cp engine/target/wasm32-unknown-unknown/release/seatlock_wasm.wasm web/public/
cp engine/target/wasm32-unknown-unknown/release/seatlock_rt.wasm web/public/
cp designs/examples/*.seatlock.json web/public/examples/

(
  cd web
  if [[ ! -d node_modules ]]; then
    npm install
  fi
)

echo "Starting the lab..."
(
  cd web
  npm run dev &
  DEV_PID=$!
  sleep 2
  xdg-open "http://127.0.0.1:5173/" >/dev/null 2>&1 || true
  wait "$DEV_PID"
)
