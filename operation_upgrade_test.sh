#!/usr/bin/env bash

die() {
    echo "$@" 1>&2
    exit 1
}

if [ $(uname -s) != "Linux" ] || [ $(uname -m) != "x86_64" ]; then
    die "This test is intended to run on x86_64 Linux"
fi

export TEST_ROOT="/tmp/hoard"
export ARCHIVE_ROOT="${TEST_ROOT}/archives"
export BIN_ROOT="${TEST_ROOT}/bin"
export XDG_CONFIG_HOME="${TEST_ROOT}/config"
export XDG_DATA_HOME="${TEST_ROOT}/data"
export HOARD_CONFIG_DIR="${XDG_CONFIG_HOME}/hoard"
export HOARD_DATA_DIR="${XDG_DATA_HOME}/hoard"
export HOARD_FILES="${TEST_ROOT}/files"
export CONFIG_FILE="${HOARD_CONFIG_DIR}/config.toml"
export HOARD_LOG="trace"

call_hoard() {
    version="$1"
    shift
    args=(--config-file "${CONFIG_FILE}" "${@}")
    if [ "${version}" = "cargo" ]; then
        cargo run -- "${args[@]}"
    else
        "${BIN_ROOT}/hoard-${version}" "${args[@]}"
    fi
}

download_hoard() {
    version="$1"
    mkdir -p "${BIN_ROOT}"
    mkdir -p "${ARCHIVE_ROOT}"
    curl -L -o "${ARCHIVE_ROOT}/hoard-${version}.tar.gz" "https://github.com/Shadow53/hoard/releases/download/${version}/hoard-x86_64-unknown-linux-gnu.tar.gz"
    tar -xzvf "${ARCHIVE_ROOT}/hoard-${version}.tar.gz" -C "${BIN_ROOT}" hoard
    mv "${BIN_ROOT}/hoard" "${BIN_ROOT}/hoard-${version}"
    chmod +x "${BIN_ROOT}/hoard-${version}"
}

reset_file() {
    mkdir -p "$(dirname "$1")"
    dd bs=1M count=1 if=/dev/urandom of="$1"
}

reset_files() {
    reset_file "${HOARD_FILES}/anon_file"
    reset_file "${HOARD_FILES}/named_file"
    reset_file "${HOARD_FILES}/anon_dir/some_file"
    reset_file "${HOARD_FILES}/anon_dir/some_dir/another_file"
    reset_file "${HOARD_FILES}/named_dir/some_file"
    reset_file "${HOARD_FILES}/named_dir/some_dir/another_file"
}

run_hoard_version() {
    version="$1"
    reset_files
    if [ "${version}" != "cargo" ]; then
        download_hoard "${version}"
    fi

    if [ "${version}" != "v0.4.0" ]; then
        if ! call_hoard "${version}" upgrade; then
            die "first upgrade command failed"
        fi

        # Run again to make sure upgrading from the newest version also works
        if ! call_hoard "${version}" upgrade; then
            die "second upgrade command failed"
        fi
    fi

    if ! call_hoard "${version}" backup; then
        die "backup command failed"
    fi
}

rm -rf "${TEST_ROOT}"

mkdir -p "${HOARD_CONFIG_DIR}"
mkdir -p "${HOARD_DATA_DIR}"
mkdir -p "${HOARD_FILES}"

tee "${CONFIG_FILE}" << EOF
[envs.always]
    env = [{ var = "HOARD_FILES" }]

[hoards.anon_file]
    "always" = "${HOARD_FILES}/anon_file"

[hoards.anon_dir]
    "always" = "${HOARD_FILES}/anon_dir"

[hoards.named]
[hoards.named.file]
    "always" = "${HOARD_FILES}/named_file"
[hoards.named.dir]
    "always" = "${HOARD_FILES}/named_dir"
EOF

run_hoard_version "v0.4.0"
read -p "Paused..." unused
run_hoard_version "cargo"

rm -rf "${TEST_ROOT}"

echo "Success!"
