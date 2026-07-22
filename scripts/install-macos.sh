#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer currently supports macOS only." >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required. Install it from https://brew.sh and run this script again." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust 1.90 or newer is required. Install it from https://rustup.rs and run this script again." >&2
  exit 1
fi

rust_version="$(rustc --version | awk '{print $2}')"
rust_major="${rust_version%%.*}"
rust_minor="${rust_version#*.}"
rust_minor="${rust_minor%%.*}"
if ((rust_major < 1 || (rust_major == 1 && rust_minor < 90))); then
  echo "Rust 1.90 or newer is required; found $rust_version." >&2
  echo "Update with: rustup update stable" >&2
  exit 1
fi

missing_formulae=()

if ! command -v pkg-config >/dev/null 2>&1; then
  missing_formulae+=(pkgconf)
fi

if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists 'chafa >= 1.8.0'; then
  missing_formulae+=(chafa)
fi

if ! command -v mpv >/dev/null 2>&1; then
  missing_formulae+=(mpv)
fi

if ((${#missing_formulae[@]} > 0)); then
  echo "Installing missing dependencies: ${missing_formulae[*]}"
  brew install "${missing_formulae[@]}"
fi

if ! pkg-config --exists 'chafa >= 1.8.0'; then
  echo "chafa was installed but pkg-config still cannot find chafa.pc." >&2
  echo "Try: export PKG_CONFIG_PATH=\"$(brew --prefix chafa)/lib/pkgconfig:\$PKG_CONFIG_PATH\"" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo install --path "$repo_root" --locked --force

cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
legacy_binary="$cargo_bin/moviebox-tui"
if [[ -e "$legacy_binary" && ! -L "$legacy_binary" ]]; then
  legacy_backup="$legacy_binary.legacy"
  if [[ ! -e "$legacy_backup" ]]; then
    mv "$legacy_binary" "$legacy_backup"
    echo "Backed up the old moviebox-tui binary to $legacy_backup"
  fi
fi
ln -sfn "$cargo_bin/moviebox" "$legacy_binary"

case ":$PATH:" in
  *":$cargo_bin:"*) ;;
  *)
    shell_profile="$HOME/.profile"
    case "${SHELL:-}" in
      */zsh) shell_profile="$HOME/.zshrc" ;;
      */bash) shell_profile="$HOME/.bashrc" ;;
    esac
    path_line='export PATH="$HOME/.cargo/bin:$PATH"'
    if [[ ! -f "$shell_profile" ]] || ! grep -Fqx "$path_line" "$shell_profile"; then
      printf '\n%s\n' "$path_line" >>"$shell_profile"
      echo "Added ~/.cargo/bin to PATH in $shell_profile"
    fi
    export PATH="$cargo_bin:$PATH"
    ;;
esac

config_root="${XDG_CONFIG_HOME:-$HOME/.config}/moviebox-tui"
subdl_key_file="$config_root/subdl_api_key"
if [[ -z "${SUBDL_API_KEY:-}" && ! -s "$subdl_key_file" && -t 0 ]]; then
  printf 'SubDL API key (recommended; press Enter to skip): '
  IFS= read -rs subdl_key
  printf '\n'
  if [[ -n "$subdl_key" ]]; then
    umask 077
    mkdir -p "$config_root"
    printf '%s\n' "$subdl_key" >"$subdl_key_file"
    echo "Saved SubDL API key to $subdl_key_file"
  fi
fi

key_file="$config_root/opensubtitles_api_key"
if [[ -z "${OPENSUBTITLES_API_KEY:-}" && ! -s "$key_file" && -t 0 ]]; then
  printf 'OpenSubtitles API key (optional fallback; press Enter to skip): '
  IFS= read -rs opensubtitles_key
  printf '\n'
  if [[ -n "$opensubtitles_key" ]]; then
    umask 077
    mkdir -p "$config_root"
    printf '%s\n' "$opensubtitles_key" >"$key_file"
    echo "Saved OpenSubtitles API key to $key_file"
  fi
fi

echo "MovieBox installed successfully. Run: moviebox"
