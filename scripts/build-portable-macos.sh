#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
config_root="${XDG_CONFIG_HOME:-$HOME/.config}/moviebox-tui"
subdl_key="$config_root/subdl_api_key"
opensubtitles_key="$config_root/opensubtitles_api_key"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "Portable package builds currently require an Apple Silicon Mac." >&2
  exit 1
fi
if [[ ! -s "$subdl_key" || ! -s "$opensubtitles_key" ]]; then
  echo "Both SubDL and OpenSubtitles key files must be configured first." >&2
  exit 1
fi

output="${1:-$HOME/Desktop/MovieBox-macOS-arm64-$(date +%Y%m%d).zip}"
if [[ -e "$output" ]]; then
  echo "Output already exists: $output" >&2
  exit 1
fi

cargo build --manifest-path "$repo_root/Cargo.toml" --release --locked

staging_root="$(mktemp -d)"
trap 'rm -rf "$staging_root"' EXIT
bundle_name="MovieBox-macOS-arm64"
bundle_root="$staging_root/$bundle_name"
mkdir -p "$bundle_root/bin" "$bundle_root/keys"

install -m 755 "$repo_root/target/release/moviebox" "$bundle_root/bin/moviebox"
install -m 755 "$repo_root/scripts/install-portable-macos.command" "$bundle_root/Kur-MovieBox.command"
install -m 600 "$subdl_key" "$bundle_root/keys/subdl_api_key"
install -m 600 "$opensubtitles_key" "$bundle_root/keys/opensubtitles_api_key"
install -m 644 "$repo_root/PORTABLE-MACOS-README.txt" "$bundle_root/OKU-BENI.txt"
install -m 644 "$repo_root/LICENSE" "$bundle_root/LICENSE"

echo "ZIP parolasını iki kez girin."
(
  cd "$staging_root"
  zip -er "$output" "$bundle_name"
)

shasum -a 256 "$output" >"$output.sha256"
echo "Portable package created: $output"
echo "Checksum created: $output.sha256"
