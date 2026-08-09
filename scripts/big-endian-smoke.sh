#!/usr/bin/env bash
set -euo pipefail

readonly cross_version="cross 0.2.5"
readonly image="ghcr.io/cross-rs/s390x-unknown-linux-gnu:0.2.5@sha256:0d8edc92c39158abe211fddfc9617bf5b7865c2ae0f58873f9cf0c58bb315271"
readonly target="s390x-unknown-linux-gnu"

cross_version_output="$(cross --version)"
actual_cross_version="${cross_version_output%%$'\n'*}"
if [[ "${actual_cross_version}" != "${cross_version}" ]]; then
  printf 'expected %s, got %s\n' "${cross_version}" "${actual_cross_version}" >&2
  exit 1
fi

printf 'cross_version=%s\n' "${actual_cross_version}"
printf 'container_image=%s\n' "${image}"
printf 'target=%s\n' "${target}"
printf 'execution_mode=qemu-user-emulation\n'

cross +1.97.0 test \
  --target "${target}" \
  --package rustmatch \
  --all-features \
  --test big_endian_smoke \
  -- --nocapture

cross +1.97.0 test \
  --target "${target}" \
  --package rustmatch-simd \
  tests::unavailable_kernel_returns_none_without_writing \
  -- --exact --nocapture
