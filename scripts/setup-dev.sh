#!/usr/bin/env sh
# Sets up a Linux machine to build and test this workspace: checks the Rust
# toolchain (with rustfmt and clippy), then downloads the same prebuilt LLVM
# release CI uses into a per-user cache, so every clone and worktree shares
# one copy. Safe to re-run: an LLVM install already in place is reused.
#
# Usage: ./scripts/setup-dev.sh [--check]
set -eu

usage() {
    cat <<'EOF'
Usage: ./scripts/setup-dev.sh [--check]

Installs the LLVM release CI uses and prints the environment variables
cargo needs to find it.

Options:
  --check    also run the CI checks (fmt, clippy, tests) once set up
  -h, --help show this help

Environment:
  DRUK_LLVM_DIR  where to install LLVM
                 (default: ${XDG_CACHE_HOME:-~/.cache}/druk/llvm-<version>)
EOF
}

die() {
    echo "Error: $*" >&2
    exit 1
}

run_checks=false
for arg in "$@"; do
    case "$arg" in
        --check) run_checks=true ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) die "unknown option '$arg' (see --help)." ;;
    esac
done

repo_root=$(cd "$(dirname "$0")/.." && pwd)

case "$(uname -s)" in
    Linux) ;;
    MINGW* | MSYS* | CYGWIN*)
        die "this script covers Linux only. On Windows, follow the manual steps in README.md (Building)."
        ;;
    *)
        die "unsupported OS '$(uname -s)'. Druk is developed on Linux and Windows only."
        ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) ;;
    *) die "unsupported CPU architecture '$(uname -m)'. Only x86_64 LLVM builds are wired up." ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
    die "cargo wasn't found on PATH. Install Rust from https://rustup.rs and re-run this script."
fi

for tool in cc curl tar xz; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        die "this script needs '$tool' on PATH, and it wasn't found."
    fi
done

if ! cargo fmt --version >/dev/null 2>&1 || ! cargo clippy --version >/dev/null 2>&1; then
    if ! command -v rustup >/dev/null 2>&1; then
        die "cargo fmt and cargo clippy are required, and rustup isn't available to install them."
    fi
    echo "Installing the rustfmt and clippy components..."
    rustup component add rustfmt clippy
fi

llvm_version=$(sed -n 's/^ *LLVM_VERSION: *//p' "$repo_root/.github/workflows/ci.yml" | head -n 1 | tr -d "\"' \r")
if [ -z "$llvm_version" ]; then
    die "couldn't read LLVM_VERSION from .github/workflows/ci.yml."
fi

llvm_dir="${DRUK_LLVM_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/druk/llvm-$llvm_version}"

is_installed() {
    [ -x "$llvm_dir/bin/llc" ] &&
        [ "$("$llvm_dir/bin/llvm-config" --version 2>/dev/null || true)" = "$llvm_version" ]
}

if is_installed; then
    echo "LLVM $llvm_version is already installed at $llvm_dir."
else
    if [ -e "$llvm_dir" ]; then
        die "$llvm_dir exists but doesn't hold LLVM $llvm_version. Remove it, or set DRUK_LLVM_DIR to another location."
    fi

    asset="LLVM-$llvm_version-Linux-X64.tar.xz"
    url="https://github.com/llvm/llvm-project/releases/download/llvmorg-$llvm_version/$asset"
    parent_dir=$(dirname "$llvm_dir")
    mkdir -p "$parent_dir"

    tmp_dir=$(mktemp -d "$parent_dir/.llvm-download.XXXXXX")
    trap 'rm -rf "$tmp_dir"' EXIT
    trap 'exit 130' INT TERM

    echo "Downloading $asset (about 2 GB)..."
    curl -fL --progress-bar -o "$tmp_dir/$asset" "$url"

    echo "Extracting into $llvm_dir..."
    mkdir "$tmp_dir/llvm"
    tar -xf "$tmp_dir/$asset" -C "$tmp_dir/llvm" --strip-components=1
    mv "$tmp_dir/llvm" "$llvm_dir"

    if ! is_installed; then
        die "the extracted archive doesn't contain a working LLVM $llvm_version."
    fi
fi

system_libs=$("$llvm_dir/bin/llvm-config" --link-static --system-libs)
# shellcheck disable=SC2086
if ! printf 'int main(void) { return 0; }\n' | cc -x c - -o /dev/null $system_libs 2>/dev/null; then
    echo "" >&2
    echo "Error: LLVM links against these system libraries, and some are missing:" >&2
    echo "  $system_libs" >&2
    echo "On Debian/Ubuntu, install them with:" >&2
    echo "  sudo apt install zlib1g-dev libzstd-dev libxml2-dev" >&2
    echo "Then re-run this script." >&2
    exit 1
fi

echo ""
echo "LLVM $llvm_version is ready."

shell_configured=false
if [ "${LLVM_SYS_221_PREFIX:-}" = "$llvm_dir" ]; then
    case ":$PATH:" in
        *":$llvm_dir/bin:"*) shell_configured=true ;;
    esac
fi

if [ "$shell_configured" = true ]; then
    echo "Your shell already points cargo at it."
else
    echo "Add these lines to your shell profile (~/.bashrc, ~/.zshrc, ...) and open a new terminal:"
    echo ""
    echo "  export LLVM_SYS_221_PREFIX=\"$llvm_dir\""
    echo "  export PATH=\"\$LLVM_SYS_221_PREFIX/bin:\$PATH\""
fi

if [ "$run_checks" = false ]; then
    echo ""
    echo "Then verify everything with: ./scripts/setup-dev.sh --check"
    exit 0
fi

export LLVM_SYS_221_PREFIX="$llvm_dir"
export PATH="$llvm_dir/bin:$PATH"
cd "$repo_root"

echo ""
echo "Running the CI checks..."
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo ""
echo "All checks passed. You're ready to contribute."
