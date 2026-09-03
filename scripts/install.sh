#!/bin/sh
set -eu

repository="Inspector-Butters/plow"
release_api="https://api.github.com/repos/${repository}/releases/latest"
install_temp=$(mktemp -d "${TMPDIR:-/tmp}/plow-install.XXXXXX")
trap 'rm -rf -- "$install_temp"' EXIT HUP INT TERM

if [ -n "${GITHUB_TOKEN:-}" ]; then
  curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" "$release_api" -o "$install_temp/release.json"
else
  curl -fsSL "$release_api" -o "$install_temp/release.json"
fi
asset_records=$(awk '
  /"url": "https:\/\/api.github.com\/repos\/[^"]*\/releases\/assets\/[0-9][0-9]*"/ {
    api_url = $0
    sub(/^[[:space:]]*"url": "/, "", api_url)
    sub(/".*$/, "", api_url)
  }
  /"browser_download_url": "/ {
    browser_url = $0
    sub(/^[[:space:]]*"browser_download_url": "/, "", browser_url)
    sub(/".*$/, "", browser_url)
    if (api_url != "") print api_url "|" browser_url
  }
' "$install_temp/release.json")

system=$(uname -s)
architecture=$(uname -m)

case "$system:$architecture" in
  Linux:x86_64|Linux:amd64)
    pattern='(amd64|x86_64)[.]AppImage$'
    ;;
  Linux:aarch64|Linux:arm64)
    pattern='(aarch64|arm64)[.]AppImage$'
    ;;
  Darwin:arm64|Darwin:aarch64)
    pattern='aarch64[.]dmg$'
    ;;
  Darwin:x86_64|Darwin:amd64)
    pattern='(x64|x86_64)[.]dmg$'
    ;;
  *)
    echo "Plow does not have a release bundle for $system $architecture yet." >&2
    exit 1
    ;;
esac

asset_record=$(printf '%s\n' "$asset_records" | grep -Ei "$pattern" | sed -n '1p' || true)
if [ -z "$asset_record" ]; then
  echo "The latest Plow release does not contain the expected bundle for $system $architecture." >&2
  exit 1
fi
asset_api=${asset_record%%|*}
asset_url=${asset_record#*|}

if [ "$system" = "Linux" ]; then
  destination_dir=${XDG_BIN_HOME:-"${HOME}/.local/bin"}
  mkdir -p "$destination_dir"
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    curl -fL -H "Accept: application/octet-stream" -H "Authorization: Bearer $GITHUB_TOKEN" "$asset_api" -o "$install_temp/plow.AppImage"
  else
    curl -fL "$asset_url" -o "$install_temp/plow.AppImage"
  fi
  install -m 755 "$install_temp/plow.AppImage" "$destination_dir/plow"
  echo "Installed Plow at $destination_dir/plow"
  echo "Run it with: $destination_dir/plow"
else
  download_dir=${HOME}/Downloads
  mkdir -p "$download_dir"
  destination="$download_dir/Plow-latest-$architecture.dmg"
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    curl -fL -H "Accept: application/octet-stream" -H "Authorization: Bearer $GITHUB_TOKEN" "$asset_api" -o "$destination"
  else
    curl -fL "$asset_url" -o "$destination"
  fi
  open "$destination"
  echo "Opened $destination — drag Plow into Applications to install it."
fi
