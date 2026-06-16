#!/bin/sh
# seCall installer — downloads the latest release binary and places it on PATH.
#
#   curl -fsSL https://raw.githubusercontent.com/hang-in/seCall/main/install.sh | sh
#
# Environment overrides:
#   SECALL_VERSION   Pin a release tag (e.g. v0.6.2). Default: latest release.
#   SECALL_INSTALL   Install directory. Default: $HOME/.local/bin
set -eu

REPO="hang-in/seCall"
INSTALL_DIR="${SECALL_INSTALL:-$HOME/.local/bin}"

err() {
	printf 'error: %s\n' "$1" >&2
	exit 1
}

info() {
	printf '  %s\n' "$1"
}

# Pick a downloader: curl or wget.
if command -v curl >/dev/null 2>&1; then
	dl() { curl -fsSL "$1"; }
	dl_to() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
	dl() { wget -qO- "$1"; }
	dl_to() { wget -qO "$2" "$1"; }
else
	err "curl or wget is required"
fi

# Detect target triple from uname.
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
Darwin)
	case "$arch" in
	arm64 | aarch64) target="aarch64-apple-darwin" ;;
	x86_64) target="x86_64-apple-darwin" ;;
	*) err "unsupported macOS architecture: $arch" ;;
	esac
	;;
Linux)
	err "Linux prebuilt binaries are not published yet (see seCall#105).
    Build from source instead:
      git clone https://github.com/$REPO.git && cd seCall
      cargo install --path crates/secall --no-default-features"
	;;
*)
	err "unsupported OS: $os (use install.ps1 on Windows)"
	;;
esac

# Resolve version (env override or latest release tag).
if [ "${SECALL_VERSION:-}" != "" ]; then
	version="$SECALL_VERSION"
else
	info "Resolving latest release..."
	version="$(dl "https://api.github.com/repos/$REPO/releases/latest" |
		grep '"tag_name"' | head -1 |
		sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
	[ "$version" != "" ] || err "could not determine latest release tag"
fi

asset="secall-$target.tar.gz"
url="https://github.com/$REPO/releases/download/$version/$asset"

info "Installing seCall $version ($target)"
info "From: $url"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

dl_to "$url" "$tmp/$asset" || err "download failed: $url"
tar -xzf "$tmp/$asset" -C "$tmp" || err "extraction failed"
[ -f "$tmp/secall" ] || err "binary 'secall' not found in $asset"

mkdir -p "$INSTALL_DIR"
mv "$tmp/secall" "$INSTALL_DIR/secall"
chmod +x "$INSTALL_DIR/secall"

info "Installed to: $INSTALL_DIR/secall"

# Warn if install dir is not on PATH.
case ":$PATH:" in
*":$INSTALL_DIR:"*) ;;
*)
	printf '\n'
	info "NOTE: $INSTALL_DIR is not on your PATH."
	info "Add it to your shell profile, e.g.:"
	info "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
	;;
esac

printf '\n'
info "Done. Next steps:"
info "  secall init      # interactive setup"
info "  secall --help"
