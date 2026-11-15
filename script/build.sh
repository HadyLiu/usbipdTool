#!/usr/bin/env bash
# Cross-compile UsbipdTool.exe in the prebuilt toolchain container
# (output <repo>/dist/UsbipdTool.exe). Does not rebuild the image when it already exists.
#
# Usage (any working directory):
#   ./script/build.sh          # unit tests + release exe
#   ./script/build.sh test     # tests only (Linux host target, core)
#   ./script/build.sh build    # exe only
# Rebuild the compiler image with ./script/build-image.sh when docker/Dockerfile changes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"
# shellcheck source=toolchain-image.sh
source "$SCRIPT_DIR/toolchain-image.sh"

MODE="${1:-all}"
IMAGE="${IMAGE:-$(usbipd_default_toolchain_image)}"
TARGET=x86_64-pc-windows-gnu

# CI 如需对齐版本，可设置 VERSION=0.1.2，使 Cargo / 清单 / UI 版本与标签一致
if [ -n "${VERSION:-}" ]; then
  echo "==> set Cargo.toml version to ${VERSION}"
  sed -i -E "s/^version = \"[^\"]+\"/version = \"${VERSION}\"/" Cargo.toml
fi

if ! usbipd_image_exists "$IMAGE"; then
  echo "==> toolchain image ${IMAGE} not found locally"
  echo "==> building it once via ${SCRIPT_DIR}/build-image.sh"
  echo "==> later rebuilds: run that script after changing docker/Dockerfile"
  "$SCRIPT_DIR/build-image.sh"
  if ! usbipd_image_exists "$IMAGE"; then
    echo "!! image ${IMAGE} is still missing after script/build-image.sh" >&2
    exit 1
  fi
else
  echo "==> using existing toolchain image ${IMAGE}"
fi

INNER='set -euo pipefail
export CARGO_TARGET_DIR=/src/target
if [ "$MODE" = all ] || [ "$MODE" = test ]; then
  echo "==> cargo test (core, host target)"
  cargo test --no-default-features
fi
if [ "$MODE" = all ] || [ "$MODE" = build ]; then
  echo "==> cargo build --release --target '"${TARGET}"'"
  cargo build --release --target '"${TARGET}"'
  mkdir -p dist
  cp -f target/'"${TARGET}"'/release/UsbipdTool.exe dist/UsbipdTool.exe
  if grep -aq requireAdministrator dist/UsbipdTool.exe; then
    echo "==> manifest: requireAdministrator embedded"
  else
    echo "!! manifest not found in exe" >&2; exit 1
  fi
  ls -l dist/UsbipdTool.exe
fi
chown -R "$HOST_UID:$HOST_GID" /src/target /src/dist /src/Cargo.lock 2>/dev/null || true'

echo "==> docker run (${MODE}) ${IMAGE}"
FONT_MOUNTS=()
if [ -d /mnt/c/Windows/Fonts ]; then
  FONT_MOUNTS+=(-v /mnt/c/Windows/Fonts:/mnt/c/Windows/Fonts:ro)
fi

docker run --rm \
  -v "$ROOT":/src \
  -v usbipdtool-cargo-registry:/usr/local/cargo/registry \
  -v usbipdtool-cargo-git:/usr/local/cargo/git \
  "${FONT_MOUNTS[@]}" \
  -e MODE="${MODE}" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  "${IMAGE}" bash -c "${INNER}"

echo "==> done"
