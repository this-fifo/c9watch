#!/usr/bin/env bash
#
# Build c9watch from source and install to /Applications
#
# Usage:
#   ./install.sh          # build + install + launch
#   ./install.sh --no-open  # build + install without launching
#
set -euo pipefail

APP_NAME="c9watch"
INSTALL_DIR="/Applications"
BUNDLE="src-tauri/target/release/bundle/macos/${APP_NAME}.app"

info()  { printf '\033[1;34m=>\033[0m %s\n' "$*"; }
error() { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

cd "$(dirname "$0")"

# --- Pre-flight checks ---

command -v cargo >/dev/null 2>&1 || error "Rust toolchain not found. Install via https://rustup.rs"
command -v npm >/dev/null 2>&1   || error "npm not found. Install Node.js first"

# --- Install npm deps if needed ---

if [ ! -d node_modules ]; then
  info "Installing npm dependencies..."
  npm install --silent
fi

# --- Build ---

info "Building ${APP_NAME} (release)..."
npm run tauri build

if [ ! -d "$BUNDLE" ]; then
  error "Build succeeded but app bundle not found at ${BUNDLE}"
fi

# --- Kill running instance ---

if pgrep -x "$APP_NAME" >/dev/null 2>&1; then
  info "Stopping running ${APP_NAME}..."
  pkill -x "$APP_NAME"
  sleep 1
fi

# --- Install ---

info "Installing to ${INSTALL_DIR}/${APP_NAME}.app..."
rm -rf "${INSTALL_DIR}/${APP_NAME}.app"
cp -R "$BUNDLE" "${INSTALL_DIR}/"

# --- Launch ---

if [ "${1:-}" != "--no-open" ]; then
  info "Launching ${APP_NAME}..."
  open "${INSTALL_DIR}/${APP_NAME}.app"
fi

echo ""
info "Done. ${APP_NAME} installed at ${INSTALL_DIR}/${APP_NAME}.app"
