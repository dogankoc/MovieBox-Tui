#!/usr/bin/env bash

set -euo pipefail

bundle_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
binary="$bundle_root/bin/moviebox"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Bu paket yalnızca macOS içindir." >&2
  exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "Bu paket Apple Silicon (M1/M2/M3/M4/M5) Mac içindir." >&2
  exit 1
fi

if [[ ! -x "$binary" ]]; then
  echo "Paket içindeki moviebox binary dosyası bulunamadı." >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew kuruluyor..."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  fi
fi

missing_formulae=()
if ! brew list --versions chafa >/dev/null 2>&1; then
  missing_formulae+=(chafa)
fi
if ! command -v mpv >/dev/null 2>&1; then
  missing_formulae+=(mpv)
fi
if ((${#missing_formulae[@]} > 0)); then
  echo "Bağımlılıklar kuruluyor: ${missing_formulae[*]}"
  brew install "${missing_formulae[@]}"
fi

install_bin="$HOME/.local/bin"
mkdir -p "$install_bin"
install -m 755 "$binary" "$install_bin/moviebox"
ln -sfn "$install_bin/moviebox" "$install_bin/moviebox-tui"
xattr -d com.apple.quarantine "$install_bin/moviebox" 2>/dev/null || true

config_root="${XDG_CONFIG_HOME:-$HOME/.config}/moviebox-tui"
mkdir -p "$config_root"
install -m 600 "$bundle_root/keys/subdl_api_key" "$config_root/subdl_api_key"
install -m 600 "$bundle_root/keys/opensubtitles_api_key" "$config_root/opensubtitles_api_key"

shell_profile="$HOME/.zshrc"
case "${SHELL:-}" in
  */bash) shell_profile="$HOME/.bashrc" ;;
esac
path_line='export PATH="$HOME/.local/bin:$PATH"'
if [[ ! -f "$shell_profile" ]] || ! grep -Fqx "$path_line" "$shell_profile"; then
  printf '\n%s\n' "$path_line" >>"$shell_profile"
fi

echo
echo "MovieBox başarıyla kuruldu."
echo "Yeni bir Terminal açıp şu komutu çalıştırın: moviebox"

