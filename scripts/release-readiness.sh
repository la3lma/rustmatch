#!/usr/bin/env bash

set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repository_root"

version=0.1.0
package_directory="$repository_root/target/package"
simd_archive="$package_directory/rustmatch-simd-$version.crate"
main_archive="$package_directory/rustmatch-$version.crate"
package_flags=()

if [[ ${RELEASE_ALLOW_DIRTY:-0} == 1 ]]; then
  package_flags+=(--allow-dirty)
fi

temporary_root=$(mktemp -d "${TMPDIR:-/tmp}/rustmatch-release-readiness.XXXXXX")
trap 'rm -rf "$temporary_root"' EXIT

echo "==> package license texts"
cmp LICENSE rustmatch/LICENSE
cmp LICENSE rustmatch-simd/LICENSE

echo "==> package contents: rustmatch-simd"
cargo package --package rustmatch-simd --list "${package_flags[@]}"

echo "==> package contents: rustmatch"
cargo package --package rustmatch --list "${package_flags[@]}"

echo "==> verified package: rustmatch-simd"
cargo package --package rustmatch-simd "${package_flags[@]}"

echo "==> unverified archive: rustmatch"
echo "    verification follows against the exact packaged rustmatch-simd source"
cargo package \
  --package rustmatch \
  --no-verify \
  --config "patch.crates-io.rustmatch-simd.path=\"$repository_root/rustmatch-simd\"" \
  "${package_flags[@]}"

tar -xzf "$simd_archive" -C "$temporary_root"
tar -xzf "$main_archive" -C "$temporary_root"
mkdir -p "$temporary_root/consumer/src"

cat >"$temporary_root/Cargo.toml" <<EOF
[workspace]
members = ["rustmatch-simd-$version", "rustmatch-$version", "consumer"]
resolver = "3"

[patch.crates-io]
rustmatch-simd = { path = "rustmatch-simd-$version" }
EOF

cat >"$temporary_root/consumer/Cargo.toml" <<EOF
[package]
name = "rustmatch-release-consumer"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
rustmatch = { path = "../rustmatch-$version" }
EOF

cat >"$temporary_root/consumer/src/main.rs" <<'EOF'
use rustmatch::{MatcherBuilder, PatternId, Utf16Text};

fn main() -> Result<(), rustmatch::Error> {
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(7), "cat|dog")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from("cat and dog");
    let mut spans = Vec::new();
    matcher.scan(&input, |matched| {
        spans.push((
            matched.pattern_id().get(),
            matched.span().start(),
            matched.span().end(),
        ));
    })?;
    spans.sort_unstable();
    assert_eq!(spans, [(7, 0, 3), (7, 8, 11)]);
    Ok(())
}
EOF

echo "==> normalized packaged dependency"
grep -A4 'dependencies.rustmatch-simd' "$temporary_root/rustmatch-$version/Cargo.toml"

echo "==> packaged workspace tests"
cargo test --manifest-path "$temporary_root/Cargo.toml" --workspace --all-features

echo "==> packaged release soak"
cargo test \
  --manifest-path "$temporary_root/Cargo.toml" \
  --release \
  --package rustmatch \
  --all-features \
  --test release_soak \
  -- \
  --ignored \
  --exact repeated_scans_preserve_results_and_cache_bounds

echo "==> packaged rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc \
  --manifest-path "$temporary_root/Cargo.toml" \
  --package rustmatch \
  --all-features \
  --no-deps

echo "==> clean packaged consumer"
cargo run \
  --manifest-path "$temporary_root/Cargo.toml" \
  --quiet \
  --package rustmatch-release-consumer

echo "==> package SHA-256"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$simd_archive" "$main_archive"
else
  shasum -a 256 "$simd_archive" "$main_archive"
fi

echo "release readiness passed without publishing"
