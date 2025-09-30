use anyhow::Result;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema,
    SchemaBuilder, TypeRef, Enum, EnumItem,
};
use async_graphql::Value;
use async_graphql_parser::types::{TypeDefinition, TypeKind};
use std::sync::Arc;
use sqlx::SqlitePool;

use crate::extensions::ExtensionManager;
use crate::repository::RepositoryStorage;
use super::extension_resolver::ExtensionFieldRegistry;
use super::schema_merger::SchemaMerger;

/// Build a dynamic GraphQL schema with extension support
pub fn build_dynamic_schema(
    pool: SqlitePool,
    storage: RepositoryStorage,
    extension_manager: ExtensionManager,
) -> Result<Schema> {
    let ext_manager_arc = Arc::new(extension_manager);
    let mut registry = ExtensionFieldRegistry::new(ext_manager_arc.clone());

    // Register extension fields
    registry.register_extensions()?;

    // Parse extension schemas
    let merger = SchemaMerger::new(ext_manager_arc.clone());
    let extension_data = merger.parse_extensions()?;

    let registry_arc = Arc::new(registry);

    // Create dynamic schema builder
    let mut builder = Schema::build("Query", Some("Mutation"), None);

    // Register core types
    register_core_types(&mut builder)?;

    // Register extension types first
    for type_def in &extension_data.types {
        register_type(&mut builder, type_def)?;
    }

    // Build Query object with core and extension fields
    let mut query = Object::new("Query");

    // Add core fields
    add_core_query_fields(&mut query, pool.clone(), storage.clone());

    // Add extension Query fields
    for (field_name, (ext_name, field_def)) in &extension_data.query_fields {
        add_extension_field(&mut query, field_name, field_def, registry_arc.clone(), "Query")?;
    }

    builder = builder.register(query);

    // Build Mutation object with core and extension fields
    let mut mutation = Object::new("Mutation");

    // Add core mutation fields
    add_core_mutation_fields(&mut mutation, pool.clone(), storage.clone());

    // Add extension Mutation fields
    for (field_name, (ext_name, field_def)) in &extension_data.mutation_fields {
        add_extension_field(&mut mutation, field_name, field_def, registry_arc.clone(), "Mutation")?;
    }

    builder = builder.register(mutation);

    // Add data to schema
    let schema = builder
        .data(pool)
        .data(storage)
        .data(ext_manager_arc)
        .data(registry_arc)
        .finish()?;

    Ok(schema)
}

/// Register core GraphQL types
fn register_core_types(builder: &mut SchemaBuilder) -> Result<()> {
    // GroupNode
    let group_node = Object::new("GroupNode")
        .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let id = node.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing id field"))?;
                Ok(Some(Value::String(id.to_string())))
            })
        }))
        .field(Field::new("slug", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let slug = node.get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing slug field"))?;
                Ok(Some(Value::String(slug.to_string())))
            })
        }))
        .field(Field::new("parent", TypeRef::named("GroupSummary"), |ctx| {
            FieldFuture::new(async move {
                // Parent resolution would happen here
                Ok(None)
            })
        }))
        .field(Field::new("repositories", TypeRef::named_list_nn("RepositorySummary"), |ctx| {
            FieldFuture::new(async move {
                // Repository resolution would happen here
                Ok(Some(Value::List(Vec::new()).into()))
            })
        }));

    // GroupSummary
    let group_summary = Object::new("GroupSummary")
        .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let id = node.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing id field"))?;
                Ok(Some(Value::String(id.to_string())))
            })
        }))
        .field(Field::new("slug", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let slug = node.get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing slug field"))?;
                Ok(Some(Value::String(slug.to_string())))
            })
        }));

    // RepositoryNode
    let repository_node = Object::new("RepositoryNode")
        .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let id = node.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing id field"))?;
                Ok(Some(Value::String(id.to_string())))
            })
        }))
        .field(Field::new("slug", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let slug = node.get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing slug field"))?;
                Ok(Some(Value::String(slug.to_string())))
            })
        }))
        .field(Field::new("group", TypeRef::named("GroupSummary"), |ctx| {
            FieldFuture::new(async move {
                // Group resolution would happen here
                Ok(None)
            })
        }))
        .field(Field::new("isRemote", TypeRef::named_nn(TypeRef::BOOLEAN), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let is_remote = node.get("remote_url").is_some();
                Ok(Some(Value::Boolean(is_remote).into()))
            })
        }))
        .field(Field::new("remoteUrl", TypeRef::named(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let url = node.get("remote_url")
                    .and_then(|v| v.as_str());
                Ok(url.map(|u| Value::String(u.to_string()).into()))
            })
        }));

    // RepositorySummary
    let repository_summary = Object::new("RepositorySummary")
        .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let id = node.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing id field"))?;
                Ok(Some(Value::String(id.to_string())))
            })
        }))
        .field(Field::new("slug", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let slug = node.get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing slug field"))?;
                Ok(Some(Value::String(slug.to_string())))
            })
        }))
        .field(Field::new("isRemote", TypeRef::named_nn(TypeRef::BOOLEAN), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let is_remote = node.get("remote_url").is_some();
                Ok(Some(Value::Boolean(is_remote).into()))
            })
        }));

    // RepositoryEntry and related types
    let entry_type_enum = Enum::new("EntryType")
        .item(EnumItem::new("FILE"))
        .item(EnumItem::new("DIRECTORY"));

    let repository_entry = Object::new("RepositoryEntry")
        .field(Field::new("name", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let name = node.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing name field"))?;
                Ok(Some(Value::String(name.to_string()).into()))
            })
        }))
        .field(Field::new("path", TypeRef::named_nn(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let path = node.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing path field"))?;
                Ok(Some(Value::String(path.to_string()).into()))
            })
        }))
        .field(Field::new("type", TypeRef::named_nn("EntryType"), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let entry_type = node.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| async_graphql::Error::new("Missing type field"))?;
                Ok(Some(Value::String(entry_type.to_string()).into()))
            })
        }));

    let repository_entries_payload = Object::new("RepositoryEntriesPayload")
        .field(Field::new("treePath", TypeRef::named(TypeRef::STRING), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let path = node.get("tree_path")
                    .and_then(|v| v.as_str());
                Ok(path.map(|p| Value::String(p.to_string()).into()))
            })
        }))
        .field(Field::new("entries", TypeRef::named_list_nn("RepositoryEntry"), |ctx| {
            FieldFuture::new(async move {
                let node = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let entries = node.get("entries")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|v| {
                                // Convert serde_json::Value to async_graphql::Value
                                let json_str = serde_json::to_string(v).unwrap();
                                serde_json::from_str::<Value>(&json_str).unwrap()
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Ok(Some(Value::List(entries).into()))
            })
        }));

    // Input types
    let create_group_input = InputObject::new("CreateGroupInput")
        .field(InputValue::new("slug", TypeRef::named_nn(TypeRef::STRING)))
        .field(InputValue::new("parent", TypeRef::named("ID")));

    let create_repository_input = InputObject::new("CreateRepositoryInput")
        .field(InputValue::new("slug", TypeRef::named_nn(TypeRef::STRING)))
        .field(InputValue::new("group", TypeRef::named("ID")));

    // Register all types
    *builder = std::mem::replace(builder, Schema::build("Query", None, None))
        .register(group_node)
        .register(group_summary)
        .register(repository_node)
        .register(repository_summary)
        .register(entry_type_enum)
        .register(repository_entry)
        .register(repository_entries_payload)
        .register(create_group_input)
        .register(create_repository_input);

    Ok(())
}

/// Register a type definition with the schema builder
fn register_type(builder: &mut SchemaBuilder, type_def: &TypeDefinition) -> Result<()> {
    match &type_def.kind {
        TypeKind::Object(obj_def) => {
            let mut object = Object::new(&*type_def.name.node);

            for field in &obj_def.fields {
                let field_name = &field.node.name.node;
                let field_type = convert_type(&field.node.ty)?;

                let mut dyn_field = Field::new(field_name, field_type, |ctx| {
                    FieldFuture::new(async move {
                        // This will be handled by extension resolver
                        Ok(Some(FieldValue::NULL))
                    })
                });

                // Add arguments if any
                for arg in &field.node.arguments {
                    let arg_name = &arg.node.name.node;
                    let arg_type = convert_type(&arg.node.ty)?;
                    dyn_field = dyn_field.argument(InputValue::new(arg_name, arg_type));
                }

                object = object.field(dyn_field);
            }

            *builder = std::mem::replace(builder, Schema::build("Query", None, None)).register(object);
        }
        TypeKind::Enum(enum_def) => {
            let mut enum_type = Enum::new(&type_def.name.node);

            for value in &enum_def.values {
                enum_type = enum_type.item(EnumItem::new(&value.node.value.node));
            }

            *builder = std::mem::replace(builder, Schema::build("Query", None, None)).register(enum_type);
        }
        TypeKind::InputObject(input_def) => {
            let mut input = InputObject::new(&type_def.name.node);

            for field in &input_def.fields {
                let field_name = &field.node.name.node;
                let field_type = convert_type(&field.node.ty)?;
                input = input.field(InputValue::new(field_name, field_type));
            }

            *builder = std::mem::replace(builder, Schema::build("Query", None, None)).register(input);
        }
        _ => {
            // Skip other type kinds for now
        }
    }

    Ok(())
}

/// Convert async-graphql-parser type to dynamic TypeRef string
fn convert_type(ty: &async_graphql_parser::types::Type) -> Result<TypeRef> {
    use async_graphql_parser::types::BaseType;

    match (&ty.base, ty.nullable) {
        (BaseType::Named(name), true) => Ok(TypeRef::named(name.as_str())),
        (BaseType::Named(name), false) => Ok(TypeRef::named_nn(name.as_str())),
        (BaseType::List(inner), true) => {
            let inner_type = inner.as_ref();
            match (&inner_type.base, inner_type.nullable) {
                (BaseType::Named(name), true) => Ok(TypeRef::named_list(name.as_str())),
                (BaseType::Named(name), false) => Ok(TypeRef::named_list_nn(name.as_str())),
                _ => {
                    // For nested lists, we need to build it manually
                    // This is a simplified version
                    Ok(TypeRef::named_list("Any"))
                }
            }
        }
        (BaseType::List(inner), false) => {
            let inner_type = inner.as_ref();
            match (&inner_type.base, inner_type.nullable) {
                (BaseType::Named(name), true) => Ok(TypeRef::named_nn_list(name.as_str())),
                (BaseType::Named(name), false) => Ok(TypeRef::named_nn_list_nn(name.as_str())),
                _ => {
                    // For nested lists, we need to build it manually
                    // This is a simplified version
                    Ok(TypeRef::named_nn_list("Any"))
                }
            }
        }
    }
}

/// Add an extension field to an object
fn add_extension_field(
    object: &mut Object,
    field_name: &str,
    field_def: &async_graphql_parser::types::FieldDefinition,
    registry: Arc<ExtensionFieldRegistry>,
    parent_type: &str,
) -> Result<()> {
    let field_type = convert_type(&field_def.ty)?;
    let parent_type_owned = parent_type.to_string();
    let field_name_owned = field_name.to_string();

    let mut field = Field::new(field_name, field_type, move |ctx| {
        let registry = registry.clone();
        let parent_type = parent_type_owned.clone();
        let field_name = field_name_owned.clone();

        FieldFuture::new(async move {
            // Extract arguments
            let mut args = async_graphql::indexmap::IndexMap::new();
            for (k, v) in ctx.args.as_index_map() {
                args.insert(async_graphql::Name::new(k.as_str()), v.clone());
            }

            let result = registry
                .resolve_field(&parent_type, &field_name, Value::Object(args), Value::Null)
                .await
                .map_err(|e| async_graphql::Error::new(e.to_string()))?;

            Ok(Some(result.into()))
        })
    });

    // Add arguments
    for arg in &field_def.arguments {
        let arg_name = &arg.node.name.node;
        let arg_type = convert_type(&arg.node.ty)?;
        field = field.argument(InputValue::new(arg_name, arg_type));
    }

    *object = std::mem::replace(object, Object::new("tmp")).field(field);

    Ok(())
}

/// Add core Query fields
fn add_core_query_fields(query: &mut Object, pool: SqlitePool, storage: RepositoryStorage) {
    // getAllGroups
    *query = query.clone().field(Field::new(
        "getAllGroups",
        TypeRef::named_list_nn("GroupNode"),
        move |ctx| {
            let pool = pool.clone();
            FieldFuture::new(async move {
                let groups = crate::group::queries::get_all_groups_raw(&pool).await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                let json = serde_json::to_value(groups)
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                let value: Value = serde_json::from_value(json)
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                Ok(Some(value.into()))
            })
        },
    ));

    // getAllRepositories
    let pool_clone = pool.clone();
    *query = query.clone().field(Field::new(
        "getAllRepositories",
        TypeRef::named_list_nn("RepositoryNode"),
        move |ctx| {
            let pool = pool_clone.clone();
            FieldFuture::new(async move {
                let repos = crate::repository::queries::get_all_repositories_raw(&pool).await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                let json = serde_json::to_value(repos)
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                let value: Value = serde_json::from_value(json)
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                Ok(Some(value.into()))
            })
        },
    ));

    // getGroup
    let pool_clone = pool.clone();
    *query = query.clone().field(
        Field::new(
            "getGroup",
            TypeRef::named("GroupNode"),
            move |ctx| {
                let pool = pool_clone.clone();
                FieldFuture::new(async move {
                    let path = ctx.args.try_get("path")?
                        .string()?;

                    let group = crate::group::queries::get_group_raw(&pool, path.to_string()).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    if let Some(group) = group {
                        let json = serde_json::to_value(group)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        let value: Value = serde_json::from_value(json)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        Ok(Some(value.into()))
                    } else {
                        Ok(None)
                    }
                })
            },
        )
        .argument(InputValue::new("path", TypeRef::named_nn(TypeRef::STRING)))
    );

    // getRepository
    let pool_clone = pool.clone();
    *query = query.clone().field(
        Field::new(
            "getRepository",
            TypeRef::named("RepositoryNode"),
            move |ctx| {
                let pool = pool_clone.clone();
                FieldFuture::new(async move {
                    let path = ctx.args.try_get("path")?
                        .string()?;

                    let repo = crate::repository::queries::get_repository_raw(&pool, path.to_string()).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    if let Some(repo) = repo {
                        let json = serde_json::to_value(repo)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        let value: Value = serde_json::from_value(json)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        Ok(Some(value.into()))
                    } else {
                        Ok(None)
                    }
                })
            },
        )
        .argument(InputValue::new("path", TypeRef::named_nn(TypeRef::STRING)))
    );

    // browseRepository
    let pool_clone = pool.clone();
    let storage_clone = storage.clone();
    *query = query.clone().field(
        Field::new(
            "browseRepository",
            TypeRef::named("RepositoryEntriesPayload"),
            move |ctx| {
                let pool = pool_clone.clone();
                let storage = storage_clone.clone();
                FieldFuture::new(async move {
                    let path = ctx.args.try_get("path")?
                        .string()?;
                    let tree_path = ctx.args.get("treePath")
                        .and_then(|v| v.string().ok())
                        .map(|s| s.to_string());

                    let entries = crate::repository::queries::browse_repository_raw(&pool, &storage, path.to_string(), tree_path).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    if let Some(entries) = entries {
                        let json = serde_json::to_value(entries)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        let value: Value = serde_json::from_value(json)
                            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                        Ok(Some(value.into()))
                    } else {
                        Ok(None)
                    }
                })
            },
        )
        .argument(InputValue::new("path", TypeRef::named_nn(TypeRef::STRING)))
        .argument(InputValue::new("treePath", TypeRef::named("String")))
    );
}

/// Add core Mutation fields
fn add_core_mutation_fields(mutation: &mut Object, pool: SqlitePool, storage: RepositoryStorage) {
    // createGroup
    *mutation = mutation.clone().field(
        Field::new(
            "createGroup",
            TypeRef::named_nn("GroupNode"),
            move |ctx| {
                let pool = pool.clone();
                FieldFuture::new(async move {
                    let input = ctx.args.try_get("input")?;

                    // Convert input to CreateGroupInput
                    let json = serde_json::to_value(input)?;
                    let create_input: crate::group::CreateGroupInput = serde_json::from_value(json)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let group = crate::group::mutations::create_group_raw(&pool, create_input).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let json = serde_json::to_value(group)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let value: Value = serde_json::from_value(json)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    Ok(Some(value.into()))
                })
            },
        )
        .argument(InputValue::new("input", TypeRef::named_nn("CreateGroupInput")))
    );

    // createRepository
    let pool_clone = pool.clone();
    let storage_clone = storage.clone();
    *mutation = mutation.clone().field(
        Field::new(
            "createRepository",
            TypeRef::named_nn("RepositoryNode"),
            move |ctx| {
                let pool = pool_clone.clone();
                let storage = storage_clone.clone();
                FieldFuture::new(async move {
                    let input = ctx.args.try_get("input")?;

                    // Convert input to CreateRepositoryInput
                    let json = serde_json::to_value(input)?;
                    let create_input: crate::repository::CreateRepositoryInput = serde_json::from_value(json)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let repo = crate::repository::mutations::create_repository_raw(&pool, &storage, create_input).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let json = serde_json::to_value(repo)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let value: Value = serde_json::from_value(json)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    Ok(Some(value.into()))
                })
            },
        )
        .argument(InputValue::new("input", TypeRef::named_nn("CreateRepositoryInput")))
    );

    // linkRemoteRepository
    let pool_clone = pool.clone();
    let storage_clone = storage.clone();
    *mutation = mutation.clone().field(
        Field::new(
            "linkRemoteRepository",
            TypeRef::named_nn("RepositoryNode"),
            move |ctx| {
                let pool = pool_clone.clone();
                let storage = storage_clone.clone();
                FieldFuture::new(async move {
                    let url = ctx.args.try_get("url")?
                        .string()?;

                    let repo = crate::repository::mutations::link_remote_repository_raw(&pool, &storage, url.to_string()).await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let json = serde_json::to_value(repo)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    let value: Value = serde_json::from_value(json)
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    Ok(Some(value.into()))
                })
            },
        )
        .argument(InputValue::new("url", TypeRef::named_nn(TypeRef::STRING)))
    );
}