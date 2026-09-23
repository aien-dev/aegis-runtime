#!/bin/sh
set -eu

# AEGIS native binary installer
# Pure compiled Rust runtime for sovereign AI agents.

REPO="aien-dev/aegis-runtime"
BIN_NAME="aegis"
INSTALL_DIR="${AEGIS_INSTALL_DIR:-$HOME/.local/bin}"
STAGED_DIR="${AEGIS_STAGED_DIR:-}"

# Parse optional command-line arguments
while [ $# -gt 0 ]; do
  case "$1" in
    --staged)
      STAGED_DIR="$2"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: install.sh [--staged <dir>] [--install-dir <dir>]"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

OS="$(uname -s)"
if [ "$OS" != "Linux" ]; then
  echo "Error: Operating system $OS is not supported. Linux is required." >&2
  exit 1
fi

RAW_ARCH="$(uname -m)"
case "$RAW_ARCH" in
  aarch64|arm64)
    ARCH="aarch64"
    ;;
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  *)
    echo "Error: Architecture $RAW_ARCH is not supported." >&2
    exit 1
    ;;
esac

echo "[+] Platform: Linux ($ARCH)"

TARBALL="aegis-linux-${ARCH}.tar.gz"
CHECKSUM_FILE="${TARBALL}.sha256"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

if [ -n "$STAGED_DIR" ]; then
  echo "[+] Installing from local staged directory: $STAGED_DIR"
  if [ -f "$STAGED_DIR/$TARBALL" ]; then
    cp "$STAGED_DIR/$TARBALL" "$TMP_DIR/$TARBALL"
    if [ -f "$STAGED_DIR/$CHECKSUM_FILE" ]; then
      cp "$STAGED_DIR/$CHECKSUM_FILE" "$TMP_DIR/$CHECKSUM_FILE"
    fi
  else
    MATCHING="$(find "$STAGED_DIR" -maxdepth 1 -name "aegis-*-linux-${ARCH}.tar.gz" 2>/dev/null | head -n 1)"
    if [ -n "$MATCHING" ] && [ -f "$MATCHING" ]; then
      cp "$MATCHING" "$TMP_DIR/$TARBALL"
      if [ -f "${MATCHING}.sha256" ]; then
        cp "${MATCHING}.sha256" "$TMP_DIR/$CHECKSUM_FILE"
      fi
    else
      echo "Error: Could not locate $TARBALL in staged directory $STAGED_DIR" >&2
      exit 1
    fi
  fi
else
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
  DOWNLOAD_URL="${BASE_URL}/${TARBALL}"
  CHECKSUM_URL="${BASE_URL}/${CHECKSUM_FILE}"

  echo "[+] Downloading $DOWNLOAD_URL..."
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TARBALL"
    curl -fsSL "$CHECKSUM_URL" -o "$TMP_DIR/$CHECKSUM_FILE" 2>/dev/null || true
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$TMP_DIR/$TARBALL" "$DOWNLOAD_URL"
    wget -qO "$TMP_DIR/$CHECKSUM_FILE" "$CHECKSUM_URL" 2>/dev/null || true
  else
    echo "Error: Neither curl nor wget was found on PATH." >&2
    exit 1
  fi
fi

if [ -f "$TMP_DIR/$CHECKSUM_FILE" ]; then
  echo "[+] Verifying SHA256 checksum..."
  EXPECTED_HASH="$(awk '{print $1}' "$TMP_DIR/$CHECKSUM_FILE")"
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH="$(sha256sum "$TMP_DIR/$TARBALL" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_HASH="$(shasum -a 256 "$TMP_DIR/$TARBALL" | awk '{print $1}')"
  else
    echo "Warning: Neither sha256sum nor shasum found; skipping checksum verification." >&2
    ACTUAL_HASH="$EXPECTED_HASH"
  fi

  if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
    echo "Error: Checksum verification failed!" >&2
    echo "Expected: $EXPECTED_HASH" >&2
    echo "Actual:   $ACTUAL_HASH" >&2
    exit 1
  fi
  echo "[+] Checksum verified: $ACTUAL_HASH"
else
  echo "[*] No checksum file provided; proceeding with extraction."
fi

echo "[+] Extracting $TARBALL..."
tar -xzf "$TMP_DIR/$TARBALL" -C "$TMP_DIR"

if [ ! -f "$TMP_DIR/$BIN_NAME" ]; then
  echo "Error: Binary $BIN_NAME not found in tarball archive." >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
cp "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "[+] Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH."
    echo "To enable direct execution, add the following to your ~/.bashrc or ~/.profile:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
    ;;
esac

echo "[+] Verification:"
"$INSTALL_DIR/$BIN_NAME" --help | head -n 2
echo "[+] Installation complete."
