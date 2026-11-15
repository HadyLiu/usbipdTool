#!/usr/bin/env bash
# Build (or rebuild) the Linux Rust+MinGW cross-compile toolchain image.
# Use this when docker/Dockerfile or .dockerignore changes. It does not compile the app
# and does not run the Windows GUI.
#
#   ./script/build-image.sh              # build the default local tag
#   ./script/build-image.sh --print-image
#   ./script/build-image.sh --print-fingerprint
#   IMAGE=ghcr.io/owner/repo:toolchain-<fp> ./script/build-image.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"
# shellcheck source=toolchain-image.sh
source "$SCRIPT_DIR/toolchain-image.sh"

usage() {
  sed -n '2,10p' "$0"
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
  --print-fingerprint)
    usbipd_toolchain_fingerprint
    exit 0
    ;;
  --print-image)
    echo "${IMAGE:-$(usbipd_default_toolchain_image)}"
    exit 0
    ;;
  "")
    ;;
  *)
    echo "unknown argument: $1" >&2
    usage >&2
    exit 1
    ;;
esac

IMAGE="${IMAGE:-$(usbipd_default_toolchain_image)}"
usbipd_build_toolchain_image "$IMAGE"
echo "==> built ${IMAGE}"
docker image inspect --format 'id={{.Id}} created={{.Created}}' "$IMAGE"
echo "==> done"
