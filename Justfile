set shell := ["bash", "-cu"]

default: server

# Run the forge server with a SQLite root directory (defaults to ./.forge/db).
server FORGE_DB_PATH='./.forge/db':
    mkdir -p {{FORGE_DB_PATH}}
    FORGE_DB_PATH={{FORGE_DB_PATH}} cargo run --manifest-path server/Cargo.toml --bin server
