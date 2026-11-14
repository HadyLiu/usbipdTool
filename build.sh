#!/usr/bin/env bash
# 在 Docker 容器中交叉编译 UsbipdTool.exe（输出 dist/UsbipdTool.exe）
#
# 用法：
#   ./build.sh          # 单元测试 + 编译 release exe
#   ./build.sh test     # 只跑单元测试（Linux 主机目标，核心层）
#   ./build.sh build    # 只编译 exe
set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"
MODE="${1:-all}"
IMAGE="${IMAGE:-usbipdtool-build:latest}"
TARGET=x86_64-pc-windows-gnu

# CI 推送 tag（如 v0.1.0）时设置 VERSION=0.1.0，使 Cargo / 清单 / UI 版本与 tag 一致
if [ -n "${VERSION:-}" ]; then
  echo "==> set Cargo.toml version to ${VERSION}"
  sed -i -E "s/^version = \"[^\"]+\"/version = \"${VERSION}\"/" Cargo.toml
fi

echo "==> docker build ${IMAGE}"
# 只把 Dockerfile 作为构建上下文，避免把 target/ 发给 docker
docker build -t "${IMAGE}" - < Dockerfile

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

echo "==> docker run (${MODE})"
docker run --rm \
  -v "$PWD":/src \
  -v usbipdtool-cargo-registry:/usr/local/cargo/registry \
  -v usbipdtool-cargo-git:/usr/local/cargo/git \
  -e MODE="${MODE}" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  "${IMAGE}" bash -c "${INNER}"

echo "==> done"
