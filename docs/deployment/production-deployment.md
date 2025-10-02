# Production Deployment Guide

This guide covers the steps required to deploy the Forgepoint server in a production environment.

## Environment Variables

The following environment variables are required for production deployments:

- `FORGE_DB_PATH`: The path to the SQLite database file.
- `FORGE_REPOS_PATH`: The path to the directory where Git repositories are stored.
- `FORGE_EXTENSIONS_DIR`: The path to the directory where extensions are loaded from.
- `ATPROTO_CLIENT_ID`: The client ID for ATProto OAuth authentication.
- `ATPROTO_CLIENT_SECRET`: The client secret for ATProto OAuth authentication.
- `ATPROTO_REDIRECT_URI`: The redirect URI for ATProto OAuth authentication.

## Database Setup

The server uses a single SQLite database file. The path to this file is specified by the `FORGE_DB_PATH` environment variable. The server will create the database file if it does not exist.

## Extension Configuration

Extensions are configured in the `forge.ron` file. The path to this file is specified by the `FORGE_CONFIG_PATH` environment variable. If this variable is not set, the server will look for a file named `forge.ron` in the current directory.

### Local Extensions

Local extensions are loaded from the filesystem. The `path` property specifies the path to the WASM file.

```ron
Config(
    extensions: Extensions(
        local: [
            LocalExtension(
                name: "issues",
                path: "./extensions/issues/api/target/wasm32-wasip1/release/forgepoint_extension_issues.wasm",
            ),
        ],
    ),
)
```

### OCI Extensions

OCI extensions are loaded from an OCI registry. The `image` property specifies the name of the image, and the `reference` property specifies the tag or digest.

```ron
Config(
    extensions: Extensions(
        oci: [
            OciExtension(
                name: "issues",
                registry: "ghcr.io",
                image: "forgepoint-dev/extensions/issues",
                reference: Tag("v1.0.0"),
            ),
        ],
    ),
)
```

## Systemd Service

Here is an example systemd service file for running the server:

```systemd
[Unit]
Description=Forgepoint Server
After=network.target

[Service]
User=forge
Group=forge
WorkingDirectory=/opt/forgepoint
Environment="FORGE_DB_PATH=/var/lib/forgepoint/db"
Environment="FORGE_REPOS_PATH=/var/lib/forgepoint/repos"
Environment="FORGE_EXTENSIONS_DIR=/var/lib/forgepoint/extensions"
ExecStart=/usr/local/bin/server
Restart=always

[Install]
WantedBy=multi-user.target
```

## Docker Compose

Here is an example Docker Compose file for running the server:

```yaml
version: "3.8"

services:
  forgepoint:
    image: ghcr.io/forgepoint-dev/forgepoint:latest
    ports:
      - "8000:8000"
    volumes:
      - ./data/db:/var/lib/forgepoint/db
      - ./data/repos:/var/lib/forgepoint/repos
      - ./extensions:/var/lib/forgepoint/extensions
    environment:
      - FORGE_DB_PATH=/var/lib/forgepoint/db
      - FORGE_REPOS_PATH=/var/lib/forgepoint/repos
      - FORGE_EXTENSIONS_DIR=/var/lib/forgepoint/extensions
```

## Monitoring and Logging

The server logs to standard output. You can use a log management tool like Fluentd or Logstash to collect and process the logs.

The server also exposes a `/metrics` endpoint for Prometheus metrics.
