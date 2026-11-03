#!/usr/bin/env bash
# Runs the benchmark tasks in benches/tasks/<task>/ through hyperfine,
# comparing every selected language that has a file for that task, then
# prints a bar chart per task and writes an HTML report.
#
# Run `bash benches/run.sh --help` for usage, and see benches/README.md
# for what these numbers do and don't mean before drawing any conclusion
# from them.
set -euo pipefail

cd "$(dirname "$0")/.."

BENCH_DIR=benches
TASKS_DIR=$BENCH_DIR/tasks
OUT_DIR=$BENCH_DIR/results
TEMPLATE=$BENCH_DIR/report.template.html

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
RUSTC=${RUSTC:-rustc}

# ---------------------------------------------------------------------------
# Language registry. To add a language, add an id to ALL_LANGS and a case to
# each lang_* function below.
# ---------------------------------------------------------------------------

ALL_LANGS=(druk-jit druk-native python lua c rust)

# Display name, used as the hyperfine command name. Must not contain commas
# (the CSV export is parsed with awk).
lang_label() {
    case $1 in
        druk-jit) echo "druk run (JIT)" ;;
        druk-native) echo "druk build (native)" ;;
        python) echo "python3" ;;
        lua) echo "lua" ;;
        c) echo "c -O2 (reference)" ;;
        rust) echo "rust -O (reference)" ;;
    esac
}

# Source file extension a task provides for this language.
lang_ext() {
    case $1 in
        druk-jit | druk-native) echo druk ;;
        python) echo py ;;
        lua) echo lua ;;
        c) echo c ;;
        rust) echo rs ;;
    esac
}

# One line per dependency: kind|value|env var|install hint.
# kind is `cmd` (looked up on PATH) or `file` (must be an executable path).
# The env var field may be empty when the tool can't be overridden.
lang_deps() {
    case $1 in
        druk-jit)
            echo "file|$DRUK|DRUK|cargo build --release -p druk"
            ;;
        druk-native)
            echo "file|$DRUK|DRUK|cargo build --release -p druk"
            # `druk build` shells out to LLVM's llc, found on PATH.
            echo "cmd|llc||LLVM, with its bin directory on PATH"
            ;;
        python) echo "cmd|$PYTHON|PYTHON|e.g. sudo apt install python3" ;;
        lua) echo "cmd|$LUA|LUA|e.g. sudo apt install lua5.4" ;;
        c) echo "cmd|$CC|CC|e.g. sudo apt install gcc" ;;
        rust) echo "cmd|$RUSTC|RUSTC|https://rustup.rs" ;;
    esac
}

# Compile step, if the language needs one. Prints the path of the produced
# binary (empty if nothing was built). $2 = source file, $3 = output stem.
lang_build() {
    case $1 in
        druk-native)
            "$DRUK" build "$2" -o "$3_druk_native" >&2 || return 1
            echo "$3_druk_native"
            ;;
        c)
            "$CC" -O2 -o "$3_c_ref" "$2" >&2 || return 1
            echo "$3_c_ref"
            ;;
        rust)
            "$RUSTC" -O -o "$3_rust_ref" "$2" >&2 || return 1
            echo "$3_rust_ref"
            ;;
    esac
}

# Command line hyperfine runs. $2 = source file, $3 = built binary (if any).
# hyperfine runs with -N (no shell), so this must not need shell features.
lang_cmd() {
    case $1 in
        druk-jit) echo "$DRUK run $2" ;;
        druk-native | c | rust) echo "$3" ;;
        python) echo "$PYTHON $2" ;;
        lua) echo "$LUA $2" ;;
    esac
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# "install: <hint>, or set <VAR>" (the second part only if VAR is set).
dep_hint() { # var hint
    if [ -n "$1" ]; then
        echo "install: $2, or set $1"
    else
        echo "install: $2"
    fi
}

dep_ok() { # kind value
    if [ "$1" = file ]; then
        [ -x "$2" ]
    else
        command -v "$2" >/dev/null 2>&1
    fi
}

# Succeeds if every dependency of the language is available. With a second
# argument `report`, prints each missing dependency to stderr.
lang_available() {
    local ok=0 kind value var hint
    while IFS='|' read -r kind value var hint; do
        if ! dep_ok "$kind" "$value"; then
            ok=1
            if [ "${2:-}" = report ]; then
                echo "  missing: $value ($(dep_hint "$var" "$hint"))" >&2
            fi
        fi
    done < <(lang_deps "$1")
    return $ok
}

all_tasks() {
    local d
    for d in "$TASKS_DIR"/*/; do
        [ -d "$d" ] && basename "$d"
    done
}

task_file() { # task lang
    echo "$TASKS_DIR/$1/$1.$(lang_ext "$2")"
}

usage() {
    cat <<EOF
Usage: bash benches/run.sh [options] [task...]

Benchmarks each task in $TASKS_DIR/ (all of them if none is given) with
hyperfine, across every selected language that has a file for that task.

Options:
  -l, --lang LIST   comma-separated languages to run (default: every
                    language whose dependencies are installed)
                    available: $(IFS=,; echo "${ALL_LANGS[*]}")
      --list        show languages, their dependencies, and tasks, then exit
      --no-report   don't write $OUT_DIR/report.html
  -h, --help        show this help

Environment: DRUK, PYTHON, LUA, CC, RUSTC override the tool used for each language.
EOF
}

list_info() {
    local lang kind value var hint mark
    echo "Languages:"
    for lang in "${ALL_LANGS[@]}"; do
        if lang_available "$lang"; then mark="✓"; else mark="✗"; fi
        printf '  %s %-12s %s\n' "$mark" "$lang" "$(lang_label "$lang")"
        while IFS='|' read -r kind value var hint; do
            if dep_ok "$kind" "$value"; then
                printf '      ✓ %s\n' "$value"
            else
                printf '      ✗ %s  (%s)\n' "$value" "$(dep_hint "$var" "$hint")"
            fi
        done < <(lang_deps "$lang")
    done
    echo
    echo "Tasks:"
    local task have
    while read -r task; do
        have=()
        for lang in "${ALL_LANGS[@]}"; do
            if [ -f "$(task_file "$task" "$lang")" ]; then
                have+=("$lang")
            fi
        done
        printf '  %-12s %s\n' "$task" "${have[*]}"
    done < <(all_tasks)
}

# Prints a bar chart from a hyperfine CSV export, fastest first. Handles both
# the hyperfine 1.x layout (command,mean,...) and the 2.x one
# (command,time_wall_clock_unit,time_wall_clock_mean,...).
print_bars() {
    awk -F, '
        NR == 1 {
            for (i = 1; i <= NF; i++) {
                if ($i == "mean" || $i == "time_wall_clock_mean") mc = i
                if ($i == "time_wall_clock_unit") uc = i
            }
            if (!mc) { print "  (no mean column in " FILENAME ")"; exit }
            next
        }
        {
            v = $mc + 0
            if (uc && $uc == "ms") v /= 1000
            if (uc && $uc == "us") v /= 1e6
            # insertion sort by mean
            for (i = n; i > 0 && mean[i] > v; i--) { mean[i + 1] = mean[i]; name[i + 1] = name[i] }
            mean[i + 1] = v; name[i + 1] = $1; n++
            if (v > max) max = v
            if (length($1) > w) w = length($1)
        }
        END {
            if (n == 0 || max <= 0) exit
            split("▏ ▎ ▍ ▌ ▋ ▊ ▉", part, " ")
            width = 40
            for (i = 1; i <= n; i++) {
                eighths = int(mean[i] / max * width * 8 + 0.5)
                bar = ""
                for (j = 0; j < int(eighths / 8); j++) bar = bar "█"
                if (eighths % 8) bar = bar part[eighths % 8]
                if (bar == "") bar = "▏"
                t = mean[i] >= 1 ? sprintf("%.2f s", mean[i]) : sprintf("%.1f ms", mean[i] * 1000)
                printf "  %-" w "s %s %s\n", name[i], bar, t
            }
        }' "$1"
}

# Builds results/report.html from the template, embedding every
# results/<task>.csv (the JSON exports are too large to embed: hyperfine 2.x
# stores every single measurement in them).
write_report() {
    local data=$OUT_DIR/.report-data.js f first=1
    {
        printf 'const BENCH_DATA = {\n'
        for f in "$OUT_DIR"/*.csv; do
            [ -f "$f" ] || continue
            [ $first -eq 1 ] || printf ',\n'
            first=0
            printf '"%s": [' "$(basename "${f%.csv}")"
            awk '{ gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); printf "%s\"%s\"", (NR > 1 ? "," : ""), $0 }' "$f"
            printf ']'
        done
        printf '};\n'
        printf 'const BENCH_META = {"generated": "%s", "host": "%s"};\n' \
            "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -srm)"
    } >"$data"
    awk -v data="$data" '
        /\/\*__BENCH_DATA__\*\// { while ((getline line < data) > 0) print line; next }
        { print }' "$TEMPLATE" >"$OUT_DIR/report.html"
    rm -f "$data"
    echo "Report: $OUT_DIR/report.html"
}

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------

requested_langs=""
report=1
tasks=()
while [ $# -gt 0 ]; do
    case $1 in
        -l | --lang)
            if [ $# -lt 2 ]; then
                echo "error: $1 needs a value" >&2
                exit 2
            fi
            requested_langs=$2
            shift 2
            ;;
        --lang=*)
            requested_langs=${1#--lang=}
            shift
            ;;
        --list)
            list_info
            exit 0
            ;;
        --no-report)
            report=0
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            echo "error: unknown option $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            tasks+=("$1")
            shift
            ;;
    esac
done

if ! command -v hyperfine >/dev/null 2>&1; then
    echo "error: hyperfine not found. Install it: cargo install hyperfine" >&2
    exit 1
fi

# Pick languages. Explicitly requested ones must be available; otherwise
# unavailable ones are skipped with a warning.
langs=()
if [ -n "$requested_langs" ]; then
    IFS=, read -r -a wanted <<<"$requested_langs"
    failed=0
    for lang in "${wanted[@]}"; do
        if [ -z "$(lang_label "$lang")" ]; then
            echo "error: unknown language '$lang' (available: $(IFS=,; echo "${ALL_LANGS[*]}"))" >&2
            exit 2
        fi
        if ! lang_available "$lang"; then
            echo "error: $lang can't run:" >&2
            lang_available "$lang" report || true
            failed=1
        fi
        langs+=("$lang")
    done
    if [ $failed -ne 0 ]; then
        exit 1
    fi
else
    for lang in "${ALL_LANGS[@]}"; do
        if lang_available "$lang"; then
            langs+=("$lang")
        else
            echo "warning: skipping $lang:" >&2
            lang_available "$lang" report || true
        fi
    done
    if [ ${#langs[@]} -eq 0 ]; then
        echo "error: no language has its dependencies installed (see --list)" >&2
        exit 1
    fi
fi

if [ ${#tasks[@]} -eq 0 ]; then
    while read -r task; do tasks+=("$task"); done < <(all_tasks)
fi

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

mkdir -p "$OUT_DIR"

built=()
cleanup_bins() {
    if [ ${#built[@]} -gt 0 ]; then
        # rustc on Windows also leaves a .pdb next to the binary.
        rm -f "${built[@]}" "${built[@]/%/.pdb}"
    fi
    built=()
}
trap cleanup_bins EXIT

ran=0
for name in "${tasks[@]}"; do
    if [ ! -d "$TASKS_DIR/$name" ]; then
        echo "skip: $name (no $TASKS_DIR/$name/ folder)" >&2
        continue
    fi

    commands=()
    for lang in "${langs[@]}"; do
        src=$(task_file "$name" "$lang")
        if [ ! -f "$src" ]; then
            continue
        fi
        if ! bin=$(lang_build "$lang" "$src" "$TASKS_DIR/$name/$name"); then
            echo "error: building $src for $lang failed" >&2
            exit 1
        fi
        if [ -n "$bin" ]; then
            built+=("$bin")
        fi
        commands+=(-n "$(lang_label "$lang")" "$(lang_cmd "$lang" "$src" "$bin")")
    done

    if [ ${#commands[@]} -eq 0 ]; then
        echo "skip: $name (no file for any selected language)" >&2
        continue
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
        --export-markdown "$OUT_DIR/${name}.md" \
        --export-json "$OUT_DIR/${name}.json" \
        --export-csv "$OUT_DIR/${name}.csv" \
        "${commands[@]}"

    echo
    echo "--- $name (mean, fastest first) ---"
    print_bars "$OUT_DIR/${name}.csv"
    echo

    cleanup_bins
    ran=$((ran + 1))
done

if [ $ran -eq 0 ]; then
    echo "error: no task ran" >&2
    exit 1
fi

echo "Results written to $OUT_DIR/*.md, *.json and *.csv"
if [ $report -eq 1 ]; then
    write_report
fi
