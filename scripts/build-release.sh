#!/usr/bin/env bash
set -euo pipefail

release_id=${1:?release id is required}
source_archive=${2:?source archive path is required}
installer=${3:?installer path is required}
unit_source=${4:?unit path is required}
root=/opt/worms-ssh
toolchain_dir="$root/toolchain"
target_dir="$root/build-target"
build_dir="/tmp/worms-ssh-build-$release_id"
release_archive="/tmp/worms-ssh-release-$release_id.tar.gz"

if ! id worms-ssh >/dev/null 2>&1; then
    useradd --system --home-dir "$root" --shell /usr/sbin/nologin worms-ssh
fi

install -d -o worms-ssh -g worms-ssh -m 0750 "$root" "$toolchain_dir" "$target_dir"

if ! command -v curl >/dev/null 2>&1 || ! command -v cc >/dev/null 2>&1; then
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl build-essential pkg-config
fi

if [[ ! -x "$toolchain_dir/cargo/bin/cargo" ]]; then
    sudo -u worms-ssh env \
        CARGO_HOME="$toolchain_dir/cargo" \
        RUSTUP_HOME="$toolchain_dir/rustup" \
        sh -c 'curl --proto "=https" --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable'
fi

rm -rf "$build_dir"
install -d -o worms-ssh -g worms-ssh -m 0750 "$build_dir"
tar -xzf "$source_archive" -C "$build_dir"
chown -R worms-ssh:worms-ssh "$build_dir"

sudo -u worms-ssh env \
    CARGO_HOME="$toolchain_dir/cargo" \
    RUSTUP_HOME="$toolchain_dir/rustup" \
    CARGO_TARGET_DIR="$target_dir" \
    "$toolchain_dir/cargo/bin/cargo" build --locked --release --manifest-path "$build_dir/Cargo.toml"

tar -czf "$release_archive" -C "$target_dir/release" worms_ssh
bash "$installer" "$release_id" "$release_archive" "$unit_source"
rm -rf "$build_dir"
rm -f "$release_archive"
