#!/usr/bin/env bash
#
# One-shot setup for SoupMashine on a Debian/Ubuntu/Linux Mint desktop.
# Installs build dependencies, the Rust toolchain (if missing), a udev rule so
# you can talk to the Maschine MK2 without root, then builds the binary.
#
#   ./setup.sh            # install deps + build
#   ./setup.sh --list     # ...and then dump the USB layout at the end
#
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

say() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m%s\033[0m\n' "$*"; }

# ---------------------------------------------------------------------------
# 1. System packages
# ---------------------------------------------------------------------------
if ! command -v apt-get >/dev/null 2>&1; then
  warn "This script targets apt-based distros (Ubuntu/Mint/Debian)."
  warn "Install these manually, then run 'cargo build --release --features full':"
  warn "  build-essential pkg-config libasound2-dev libudev-dev curl git"
  exit 1
fi

say "Installing system packages (sudo may prompt for your password)"
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config libasound2-dev libudev-dev curl git

# ---------------------------------------------------------------------------
# 2. Rust toolchain
# ---------------------------------------------------------------------------
# Pick up an existing rustup/cargo install if present.
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

if ! command -v cargo >/dev/null 2>&1; then
  say "Installing the Rust toolchain via rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
else
  say "Found cargo: $(cargo --version)"
fi

# ---------------------------------------------------------------------------
# 3. udev rule so non-root users can access the controller (vendor 0x17cc)
# ---------------------------------------------------------------------------
RULE=/etc/udev/rules.d/50-maschine.rules
if [ ! -f "$RULE" ]; then
  say "Installing udev rule for Native Instruments devices"
  echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"' | sudo tee "$RULE" >/dev/null
  sudo udevadm control --reload-rules
  sudo udevadm trigger
  warn "If the MK2 was already plugged in, unplug and replug it now."
else
  say "udev rule already present ($RULE)"
fi

# ---------------------------------------------------------------------------
# 4. Build
# ---------------------------------------------------------------------------
say "Building SoupMashine (release, full features). First build takes a few minutes."
cargo build --release --features full

BIN="$REPO_DIR/target/release/soupmashine"
say "Done. Binary: $BIN"
echo
echo "Next:"
echo "  $BIN --list      # inspect the USB layout (paste this if the splash persists)"
echo "  $BIN             # run the groovebox (plug in the MK2 first)"

# ---------------------------------------------------------------------------
# 5. Optional: show the USB layout now
# ---------------------------------------------------------------------------
if [ "${1:-}" = "--list" ]; then
  say "USB layout"
  "$BIN" --list || warn "No MK2 detected yet — plug it in and re-run: $BIN --list"
fi
