#!/bin/bash
set -e
# cpx installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/11happy/cpx/main/install.sh | bash

REPO="11happy/cpx"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="cpx"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect platform
detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64) echo "linux-x86_64-musl" ;;
                aarch64|arm64) echo "linux-aarch64-musl" ;;
                armv7l) echo "linux-armv7-musl" ;;
                *) error "Unsupported architecture: $arch" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64) echo "macos-x86_64" ;;
                arm64|aarch64) echo "macos-aarch64" ;;
                *) error "Unsupported architecture: $arch" ;;
            esac
            ;;
        *)
            error "Unsupported OS: $os (Linux and macOS are supported)"
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    local api_url="https://api.github.com/repos/$REPO/releases/latest"
    if command -v curl &> /dev/null; then
        curl -s "$api_url" | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/'
    elif command -v wget &> /dev/null; then
        wget -qO- "$api_url" | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download and install
install_cpx() {
    local platform version download_url tarball_name

    platform=$(detect_platform)
    info "Detected platform: $platform"

    version=$(get_latest_version)
    if [ -z "$version" ]; then
        error "Failed to get latest version"
    fi
    info "Latest version: v$version"

    tarball_name="cpx-${platform}.tar.gz"
    download_url="https://github.com/$REPO/releases/download/v${version}/${tarball_name}"

    info "Downloading from: $download_url"

    # Create temporary directory
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download
    if command -v curl &> /dev/null; then
        if ! curl -fsSL "$download_url" -o "$tmp_dir/$tarball_name"; then
            error "Failed to download from: $download_url"
        fi
    elif command -v wget &> /dev/null; then
        if ! wget -q "$download_url" -O "$tmp_dir/$tarball_name"; then
            error "Failed to download from: $download_url"
        fi
    fi

    # Extract
    info "Extracting to $INSTALL_DIR..."
    tar xzf "$tmp_dir/$tarball_name" -C "$tmp_dir"

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "$tmp_dir/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    info "✓ cpx v$version installed to $INSTALL_DIR/$BINARY_NAME"

    # Check if in PATH
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        warn "$INSTALL_DIR is not in your PATH"
        warn "Add this to your ~/.bashrc or ~/.zshrc:"
        echo ""
        echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi

    # Verify installation
    if "$INSTALL_DIR/$BINARY_NAME" --version &> /dev/null; then
        info "Installation verified successfully!"
        "$INSTALL_DIR/$BINARY_NAME" --version
    else
        warn "Installation completed but verification failed"
    fi
}

# Main
main() {
    echo "╔═══════════════════════════════════╗"
    echo "║   cpx - Modern File Copy Tool     ║"
    echo "╚═══════════════════════════════════╝"
    echo ""

    install_cpx

    echo ""
    echo "To get started, run: cpx --help"
}

main
