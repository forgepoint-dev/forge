# Migration Guide

This guide covers the steps required to migrate from the old extension system to the new one.

## Upgrading from main branch

The new extension system is not backwards compatible with the old one. You will need to update your `forge.ron` file to use the new configuration format.

### Old Configuration

```ron
Config(
    extensions: [
        "issues",
    ],
)
```

### New Configuration

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

## Database Migrations

There are no database migrations required for this update.

## Configuration Changes

The `extensions` property in the `forge.ron` file has been changed from a list of strings to a struct with `local` and `oci` properties.

## Rollback Procedures

To rollback to the old extension system, you will need to revert the changes to the `forge.ron` file and restart the server.
