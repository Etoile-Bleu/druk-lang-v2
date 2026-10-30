#!/usr/bin/env bash
# Runs the benches in this directory through hyperfine, comparing the
# release-built `druk` binary against Python, Lua, and a gcc -O2 build of
# the equivalent C program, for every task named on the command line (or
# every task in this directory if none is given).
#
# See benches/README.md for what these numbers do and don't mean before
# drawing any conclusion from them.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -z "${DRUK:-}" ]; then
    if [ -x ./target/release/druk.exe ]; then
        DRUK=./target/release/druk.exe
    else
        DRUK=./target/release/druk
    fi
fi
PYTHON=${PYTHON:-python3}
LUA=${LUA:-lua}
CC=${CC:-gcc}

if [ ! -x "$DRUK" ]; then
    echo "error: $DRUK not found or not executable. Build it first:" >&2
    echo "  cargo build --release -p druk" >&2
    exit 1
fi

if ! command -v hyperfine >/dev/null 2>&1; then
    echo "error: hyperfine not found. Install it: cargo install hyperfine" >&2
    exit 1
fi

tasks=("$@")
if [ ${#tasks[@]} -eq 0 ]; then
    tasks=()
    for f in benches/*.druk; do
        tasks+=("$(basename "${f%.druk}")")
    done
fi

out_dir=benches/results
mkdir -p "$out_dir"

for name in "${tasks[@]}"; do
    druk_file="benches/${name}.druk"
    py_file="benches/${name}.py"
    lua_file="benches/${name}.lua"
    c_file="benches/${name}.c"

    if [ ! -f "$druk_file" ] || [ ! -f "$py_file" ] || [ ! -f "$lua_file" ]; then
        echo "skip: $name (missing a .druk/.py/.lua file)" >&2
        continue
    fi

    c_bin=""
    cleanup_c_bin() {
        if [ -n "$c_bin" ]; then
            rm -f "$c_bin"
        fi
    }
    trap cleanup_c_bin EXIT

    commands=(
        -n "druk" "$DRUK run $druk_file"
        -n "python3" "$PYTHON $py_file"
        -n "lua" "$LUA $lua_file"
    )
    if [ -f "$c_file" ]; then
        c_bin="benches/${name}_c_ref"
        "$CC" -O2 -o "$c_bin" "$c_file"
        commands+=(-n "c (gcc -O2, reference only, see README)" "$c_bin")
    fi

    echo "=== $name ==="
    # -N (--shell=none): hyperfine splits each command string on
    # whitespace and execs it directly, with no intermediate shell. This
    # sidesteps a real cross-platform problem: hyperfine's default shell
    # resolution picked up an unrelated WSL `bash.exe` ahead of Git Bash's
    # own on a Windows PATH that has both, which in turn had no access to
    # the Lua install used for this comparison. None of these commands
    # need shell features (no pipes, redirects, or `&&`), so -N is both
    # the fix and the simpler choice.
    hyperfine \
        -N \
        --warmup 3 \
        --min-runs 20 \
        --export-markdown "$out_dir/${name}.md" \
        --export-json "$out_dir/${name}.json" \
        "${commands[@]}"

    cleanup_c_bin
    trap - EXIT
done

echo
echo "Results written to $out_dir/*.md and $out_dir/*.json"
