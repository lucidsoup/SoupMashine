#!/usr/bin/env bash
#
# Install SoupMashine as a boot service on a Debian/Raspberry Pi OS host.
# Run from the repo root:  sudo ./deploy/install.sh
#
set -euo pipefail

PREFIX="/opt/soupmashine"
SERVICE_USER="${SUDO_USER:-pi}"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $EUID -ne 0 ]]; then
  echo "Please run as root (sudo)." >&2
  exit 1
fi

echo "==> Installing build + runtime dependencies"
apt-get update
apt-get install -y build-essential pkg-config libasound2-dev libudev-dev

echo "==> Building release binary (features: full)"
sudo -u "$SERVICE_USER" bash -lc "cd '$REPO_DIR' && cargo build --release --features full"

echo "==> Installing binary to $PREFIX"
install -d "$PREFIX"
install -m 0755 "$REPO_DIR/target/release/soupmashine" "$PREFIX/soupmashine"

echo "==> Installing udev rule for Maschine MK2 (VID 17cc)"
cat >/etc/udev/rules.d/50-maschine.rules <<'EOF'
SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", ATTR{idProduct}=="1140", MODE="0666"
EOF
udevadm control --reload-rules
udevadm trigger

echo "==> Installing systemd service (user: $SERVICE_USER)"
sed "s/^User=.*/User=$SERVICE_USER/" "$REPO_DIR/deploy/soupmashine.service" \
  >/etc/systemd/system/soupmashine.service
systemctl daemon-reload
systemctl enable soupmashine.service

echo
echo "Done. Plug in the Maschine MK2, then start it with:"
echo "    sudo systemctl start soupmashine"
echo "Follow logs with:"
echo "    journalctl -u soupmashine -f"
