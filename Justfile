set shell := ["bash", "-cu"]
set dotenv-load := true

export FORGE_DB_PATH := "server/.forge/db"
export FORGE_REPOS_PATH := "server/.forge/repos"
export FORGE_EXTENSIONS_DIR := "server/extensions"

# Default task builds the issues extension and runs the server.
default: install-extension run-server

# Compile the Issues extension to WASM using the project nix shell.
build-extension:
    nix develop --impure -c cargo build --package forgepoint-extension-issues --target wasm32-wasip1 --release

# Copy the freshly built Issues extension into the server's extensions directory.
install-extension: build-extension
    install -d {{FORGE_EXTENSIONS_DIR}}
    install -m 0644 target/wasm32-wasip1/release/forgepoint_extension_issues.wasm \
        {{FORGE_EXTENSIONS_DIR}}/issues.wasm

# Run the Forge server with sqlite + repo roots under server/.forge/
run-server:
    mkdir -p {{FORGE_DB_PATH}}
    mkdir -p {{FORGE_REPOS_PATH}}
    FORGE_DB_PATH={{FORGE_DB_PATH}} FORGE_REPOS_PATH={{FORGE_REPOS_PATH}} \
        FORGE_EXTENSIONS_DIR={{FORGE_EXTENSIONS_DIR}} \
        nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server

# Run Smart HTTP v2 e2e tests (ls-remote + clone via git backend)
test-git-http-v2:
    nix develop --impure -c bash server/tests/git_http_v2_clone.sh

test-git-http-v2-shallow:
    nix develop --impure -c bash server/tests/git_http_v2_shallow.sh

test-git-http-v2-fetch:
    nix develop --impure -c bash server/tests/git_http_v2_fetch_update.sh

test-git-http-v2-concurrency:
    nix develop --impure -c bash server/tests/git_http_v2_concurrency.sh

test-git-http-v2-visibility:
    nix develop --impure -c bash server/tests/git_http_v2_visibility.sh

# Rust backend e2e (pure-Rust pack + ls-refs)
test-git-http-v2-rust:
    nix develop --impure -c bash server/tests/git_http_v2_rust_backend.sh

# Packet trace normalization + diff (baseline vs. rust)
test-git-http-v2-diff:
    nix develop --impure -c bash server/tests/git_http_v2_trace_diff.sh

# Simple clone benchmark (baseline vs. rust). Override BENCH_* envs as needed.
bench-git-http-v2:
    nix develop --impure -c bash server/tests/git_http_v2_bench.sh

# Start the Astro + Vue web client in dev mode.
run-web:
    cd apps/web && nix develop --impure -c bun run dev

# Remove local extension artifacts.
clean-extension:
    rm -f {{FORGE_EXTENSIONS_DIR}}/issues.wasm
    rm -f target/wasm32-wasip1/release/forgepoint_extension_issues.wasm

# Build the forge CLI binary (HTTP-based remote management tool)
build-cli:
    nix develop --impure -c cargo build --package forge-cli --release

# Run the forge CLI (pass arguments after --)
# Example: just run-cli repo create my-project
run-cli *ARGS:
    nix develop --impure -c cargo run --package forge-cli -- {{ARGS}}

