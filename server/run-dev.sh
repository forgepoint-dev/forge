#!/bin/bash

# Run the server in development mode with in-memory SQLite
echo "Starting Forge server in development mode with in-memory SQLite..."
echo "This mode uses:"
echo "  - In-memory SQLite database (data is not persisted)"
echo "  - Temporary directories for repositories"
echo "  - Local extensions directory"
echo ""
echo "Server will be available at http://localhost:8000"
echo "GraphQL playground at http://localhost:8000"
echo ""

# Set environment variables for in-memory mode
export FORGE_IN_MEMORY_DB=true
export FORGE_EXTENSIONS_DIR=extensions_dir

# Run the server
exec cargo run "$@"