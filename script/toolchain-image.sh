# Shared helpers for the Linux Rust+MinGW *compiler toolchain* image.
# Sourced by script/build-image.sh and script/build.sh. Not an entrypoint.
# The Windows GUI is not run inside this image; app sources are bind-mounted at compile time.

usbipd_toolchain_fingerprint() {
  (
    cd "$ROOT"
    # Filenames are included so the tag changes if the set of files moves.
    sha256sum .dockerignore docker/Dockerfile | sha256sum | awk '{print substr($1,1,16)}'
  )
}

usbipd_default_toolchain_image() {
  echo "usbipdtool-build:toolchain-$(usbipd_toolchain_fingerprint)"
}

usbipd_image_exists() {
  docker image inspect "$1" >/dev/null 2>&1
}

usbipd_docker_buildx_available() {
  docker buildx version >/dev/null 2>&1
}

usbipd_build_toolchain_image() {
  local image="$1"
  local dockerfile="$ROOT/docker/Dockerfile"
  echo "==> building toolchain image ${image}"
  echo "==> -f docker/Dockerfile (repo-root context; compile toolchain only, not the Windows GUI)"
  if usbipd_docker_buildx_available; then
    echo "==> using docker buildx build --load"
    docker buildx build --load -t "${image}" -f "$dockerfile" "$ROOT"
  else
    echo "==> docker buildx not available; using docker build"
    docker build -t "${image}" -f "$dockerfile" "$ROOT"
  fi
}
