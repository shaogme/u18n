#!/usr/bin/env bash
set -euo pipefail

workspace_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_manifest="$workspace_root/examples/keyed-size-fixture/Cargo.toml"
target_root="$workspace_root/target/keyed-size-fixture"
artifact_root="$target_root/artifacts"
mkdir -p "$artifact_root"

check_mode() {
    local mode=$1
    shift
    cargo build \
        --manifest-path "$fixture_manifest" \
        --target-dir "$target_root/build" \
        --release \
        --no-default-features \
        --features "$mode"
    local binary="$target_root/build/release/u18n-keyed-size-fixture"
    local artifact="$artifact_root/$mode"
    cp "$binary" "$artifact"
    strings "$artifact" > "$artifact.strings"
    for marker in "$@"; do
        if [[ "$marker" == "!"* ]]; then
            if grep -Fq -- "${marker#!}" "$artifact.strings"; then
                printf 'unexpected linked payload in %s: %s\n' "$mode" "${marker#!}" >&2
                return 1
            fi
        elif ! grep -Fq -- "$marker" "$artifact.strings"; then
            printf 'missing linked payload in %s: %s\n' "$mode" "$marker" >&2
            return 1
        fi
    done
    local read_only_bytes
    read_only_bytes=$(size -A "$artifact" | awk '$1 == ".rodata" { print $2 }')
    printf '%-8s .rodata=%s bytes\n' "$mode" "${read_only_bytes:-0}"
}

check_mode one \
    KEYED_ALPHA_UNIQUE_PAYLOAD_71A9 \
    '!KEYED_BETA_UNIQUE_PAYLOAD_82B0' \
    '!KEYED_GAMMA_UNIQUE_PAYLOAD_93C1' \
    '!KEYED_DELTA_UNIQUE_PAYLOAD_A4D2'
check_mode several \
    KEYED_ALPHA_UNIQUE_PAYLOAD_71A9 \
    KEYED_BETA_UNIQUE_PAYLOAD_82B0 \
    '!KEYED_GAMMA_UNIQUE_PAYLOAD_93C1' \
    '!KEYED_DELTA_UNIQUE_PAYLOAD_A4D2'
check_mode all \
    KEYED_ALPHA_UNIQUE_PAYLOAD_71A9 \
    KEYED_BETA_UNIQUE_PAYLOAD_82B0 \
    KEYED_GAMMA_UNIQUE_PAYLOAD_93C1 \
    KEYED_DELTA_UNIQUE_PAYLOAD_A4D2
check_mode dynamic \
    KEYED_ALPHA_UNIQUE_PAYLOAD_71A9 \
    KEYED_BETA_UNIQUE_PAYLOAD_82B0 \
    KEYED_GAMMA_UNIQUE_PAYLOAD_93C1 \
    KEYED_DELTA_UNIQUE_PAYLOAD_A4D2

printf 'Artifacts: %s\n' "$artifact_root"
