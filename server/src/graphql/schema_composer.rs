use anyhow::{Context, Result};
use graphql_parser::Pos;
use graphql_parser::schema::{
    Definition, Directive, Document, EnumType, EnumValue, InputObjectType, InterfaceType,
    ObjectType, ScalarType, TypeDefinition, TypeExtension, UnionType, Value,
};
use std::collections::HashMap;

/// Composes the core supergraph SDL with GraphQL federation fragments supplied by extensions.
///
/// The implementation relies on Hive Router's GraphQL tooling (`graphql_parser`) to manipulate
/// structured AST nodes instead of concatenating strings. Each extension schema is parsed into a
/// document, decorated with the required `@join__*` directives, and merged into the core
/// supergraph representation before serialising the final SDL.
pub struct SchemaComposer {
    core_schema: Document<'static, String>,
    subgraphs: HashMap<String, Document<'static, String>>,
}

impl SchemaComposer {
    pub fn new() -> Self {
        let core_schema = graphql_parser::parse_schema::<String>(CORE_SUPERGRAPH_SDL)
            .expect("core supergraph SDL must be valid")
            .into_static();

        Self {
            core_schema,
            subgraphs: HashMap::new(),
        }
    }

    pub fn add_subgraph(&mut self, name: String, schema: String) -> Result<()> {
        let document = graphql_parser::parse_schema::<String>(&schema)
            .with_context(|| format!("failed to parse schema for extension `{name}`"))?
            .into_static();

        self.subgraphs.insert(name, document);
        Ok(())
    }

    pub fn compose(&self) -> Result<String> {
        let mut supergraph = self.core_schema.clone();

        for (name, document) in &self.subgraphs {
            let graph_name = name.to_ascii_uppercase();
            tracing::debug!("Processing extension `{}`", name);
            merge_extension_into_supergraph(&mut supergraph, &graph_name, name, document)
                .with_context(|| format!("failed to merge extension `{name}`"))?;
        }

        let serialised = format!("{}", supergraph);
        tracing::debug!("Final supergraph SDL:\n{}", serialised);
        Ok(serialised)
    }
}

impl Default for SchemaComposer {
    fn default() -> Self {
        Self::new()
    }
}

const CORE_SUPERGRAPH_SDL: &str = r#"
schema
  @link(url: "https://specs.apollo.dev/link/v1.0")
  @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
  mutation: Mutation
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(
  graph: join__Graph
  requires: join__FieldSet
  provides: join__FieldSet
  type: String
  external: Boolean
  override: String
  usedOverridden: Boolean
) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(
  graph: join__Graph!
  interface: String!
) repeatable on OBJECT | INTERFACE

directive @join__type(
  graph: join__Graph!
  key: join__FieldSet
  extension: Boolean! = false
  resolvable: Boolean! = true
  isInterfaceObject: Boolean! = false
) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR

directive @join__unionMember(
  graph: join__Graph!
  member: String!
) repeatable on UNION

directive @link(
  url: String
  as: String
  for: link__Purpose
  import: [link__Import]
) repeatable on SCHEMA

scalar join__FieldSet

enum link__Purpose {
  SECURITY
  EXECUTION
}

scalar link__Import

type Query @join__type(graph: CORE) {
  # Core fields
  getAllGroups: [GroupNode!]! @join__field(graph: CORE)
  getAllRepositories: [RepositoryNode!]! @join__field(graph: CORE)
  getGroup(path: String!): GroupNode @join__field(graph: CORE)
  getRepository(path: String!): RepositoryNode @join__field(graph: CORE)
  browseRepository(path: String!, treePath: String, branch: String): RepositoryEntriesPayload @join__field(graph: CORE)
  listRepositoryBranches(path: String!): [RepositoryBranch!] @join__field(graph: CORE)
  readRepositoryFile(path: String!, filePath: String!, branch: String): RepositoryFilePayload @join__field(graph: CORE)
}

type Mutation @join__type(graph: CORE) {
  # Core mutations
  createGroup(input: CreateGroupInput!): GroupNode! @join__field(graph: CORE)
  createRepository(input: CreateRepositoryInput!): RepositoryNode! @join__field(graph: CORE)
  linkRemoteRepository(url: String!): RepositoryNode! @join__field(graph: CORE)
  cloneRepository(url: String!): RepositoryNode! @join__field(graph: CORE)
}

# Core types
type GroupNode @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
  parent: GroupSummary @join__field(graph: CORE)
  repositories: [RepositorySummary!]! @join__field(graph: CORE)
}

type GroupSummary @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
}

type RepositoryNode @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
  group: GroupSummary @join__field(graph: CORE)
  isRemote: Boolean! @join__field(graph: CORE)
  remoteUrl: String @join__field(graph: CORE)
}

type RepositorySummary @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
  isRemote: Boolean! @join__field(graph: CORE)
  remoteUrl: String @join__field(graph: CORE)
}

type RepositoryEntriesPayload @join__type(graph: CORE) {
  treePath: String @join__field(graph: CORE)
  entries: [RepositoryEntry!]! @join__field(graph: CORE)
}

type RepositoryFilePayload @join__type(graph: CORE) {
  path: String! @join__field(graph: CORE)
  name: String! @join__field(graph: CORE)
  size: Int! @join__field(graph: CORE)
  isBinary: Boolean! @join__field(graph: CORE)
  text: String @join__field(graph: CORE)
  truncated: Boolean! @join__field(graph: CORE)
}

type RepositoryEntry @join__type(graph: CORE) {
  name: String! @join__field(graph: CORE)
  path: String! @join__field(graph: CORE)
  type: EntryType! @join__field(graph: CORE)
  size: Int @join__field(graph: CORE)
}

type RepositoryBranch @join__type(graph: CORE) {
  name: String! @join__field(graph: CORE)
  reference: String! @join__field(graph: CORE)
  target: String @join__field(graph: CORE)
  isDefault: Boolean! @join__field(graph: CORE)
}

enum EntryType @join__type(graph: CORE) {
  FILE @join__enumValue(graph: CORE)
  DIRECTORY @join__enumValue(graph: CORE)
}

input CreateGroupInput @join__type(graph: CORE) {
  slug: String!
  parent: ID
}

input CreateRepositoryInput @join__type(graph: CORE) {
  slug: String!
  group: ID
}

enum join__Graph {
  CORE @join__graph(name: "core", url: "internal://core")
}
"#;

fn merge_extension_into_supergraph(
    supergraph: &mut Document<'static, String>,
    graph_name: &str,
    subgraph_name: &str,
    document: &Document<'static, String>,
) -> Result<()> {
    ensure_join_graph_entry(supergraph, graph_name, subgraph_name);

    for definition in &document.definitions {
        match definition.clone() {
            Definition::TypeDefinition(definition) => {
                let decorated = decorate_type_definition(definition, graph_name);
                supergraph
                    .definitions
                    .push(Definition::TypeDefinition(decorated));
            }
            Definition::TypeExtension(extension) => {
                apply_type_extension(supergraph, extension, graph_name)?;
            }
            Definition::DirectiveDefinition(definition) => {
                supergraph
                    .definitions
                    .push(Definition::DirectiveDefinition(definition));
            }
            Definition::SchemaDefinition(definition) => {
                merge_schema_definition(supergraph, definition);
            }
        }
    }

    Ok(())
}

fn decorate_type_definition(
    mut definition: TypeDefinition<'static, String>,
    graph_name: &str,
) -> TypeDefinition<'static, String> {
    match &mut definition {
        TypeDefinition::Object(object) => {
            ensure_join_type(&mut object.directives, graph_name);
            for field in &mut object.fields {
                ensure_join_field(&mut field.directives, graph_name);
            }
        }
        TypeDefinition::Interface(interface) => {
            ensure_join_type(&mut interface.directives, graph_name);
            for field in &mut interface.fields {
                ensure_join_field(&mut field.directives, graph_name);
            }
        }
        TypeDefinition::Enum(enum_type) => {
            ensure_join_type(&mut enum_type.directives, graph_name);
            for value in &mut enum_type.values {
                ensure_join_enum_value(&mut value.directives, graph_name);
            }
        }
        TypeDefinition::InputObject(input) => {
            ensure_join_type(&mut input.directives, graph_name);
            for field in &mut input.fields {
                ensure_join_field(&mut field.directives, graph_name);
            }
        }
        TypeDefinition::Scalar(scalar) => {
            ensure_join_type(&mut scalar.directives, graph_name);
        }
        TypeDefinition::Union(union_type) => {
            ensure_join_type(&mut union_type.directives, graph_name);
            ensure_join_union_members(&mut union_type.directives, &union_type.types, graph_name);
        }
    }

    definition
}

fn apply_type_extension(
    document: &mut Document<'static, String>,
    extension: TypeExtension<'static, String>,
    graph_name: &str,
) -> Result<()> {
    match extension {
        TypeExtension::Object(mut ext) => {
            let target = find_object_type_mut(document, ext.name.as_str())
                .with_context(|| format!("object type `{}` not found for extension", ext.name))?;
            ensure_join_type(&mut target.directives, graph_name);
            target.directives.extend(ext.directives.drain(..));
            for mut field in ext.fields.drain(..) {
                ensure_join_field(&mut field.directives, graph_name);
                target.fields.push(field);
            }
        }
        TypeExtension::Interface(mut ext) => {
            let target = find_interface_type_mut(document, ext.name.as_str())
                .with_context(|| format!("interface `{}` not found for extension", ext.name))?;
            ensure_join_type(&mut target.directives, graph_name);
            target.directives.extend(ext.directives.drain(..));
            for mut field in ext.fields.drain(..) {
                ensure_join_field(&mut field.directives, graph_name);
                target.fields.push(field);
            }
        }
        TypeExtension::InputObject(mut ext) => {
            let target = find_input_object_type_mut(document, ext.name.as_str())
                .with_context(|| format!("input object `{}` not found for extension", ext.name))?;
            ensure_join_type(&mut target.directives, graph_name);
            target.directives.extend(ext.directives.drain(..));
            for mut field in ext.fields.drain(..) {
                ensure_join_field(&mut field.directives, graph_name);
                target.fields.push(field);
            }
        }
        TypeExtension::Enum(mut ext) => {
            let target = find_enum_type_mut(document, ext.name.as_str())
                .with_context(|| format!("enum `{}` not found for extension", ext.name))?;
            ensure_join_type(&mut target.directives, graph_name);
            target.directives.extend(ext.directives.drain(..));
            for mut value in ext.values.drain(..) {
                ensure_join_enum_value(&mut value.directives, graph_name);
                target.values.push(value);
            }
        }
        TypeExtension::Union(mut ext) => {
            let target = find_union_type_mut(document, ext.name.as_str())
                .with_context(|| format!("union `{}` not found for extension", ext.name))?;
            ensure_join_type(&mut target.directives, graph_name);
            target.directives.extend(ext.directives.drain(..));
            target.types.extend(ext.types.drain(..));
            ensure_join_union_members(&mut target.directives, &target.types, graph_name);
        }
        TypeExtension::Scalar(mut ext) => {
            let target = find_scalar_type_mut(document, ext.name.as_str())
                .with_context(|| format!("scalar `{}` not found for extension", ext.name))?;
            target.directives.extend(ext.directives.drain(..));
            ensure_join_type(&mut target.directives, graph_name);
        }
    }

    Ok(())
}

fn merge_schema_definition(
    document: &mut Document<'static, String>,
    schema_definition: graphql_parser::schema::SchemaDefinition<'static, String>,
) {
    if schema_definition.directives.is_empty() {
        return;
    }

    if let Some(existing) =
        document
            .definitions
            .iter_mut()
            .find_map(|definition| match definition {
                Definition::SchemaDefinition(definition) => Some(definition),
                _ => None,
            })
    {
        existing.directives.extend(schema_definition.directives);
    } else {
        document
            .definitions
            .push(Definition::SchemaDefinition(schema_definition));
    }
}

fn ensure_join_graph_entry(
    document: &mut Document<'static, String>,
    graph_name: &str,
    subgraph_name: &str,
) {
    if let Some(enum_type) = find_enum_type_mut(document, "join__Graph") {
        if enum_type
            .values
            .iter()
            .any(|value| value.name.as_str() == graph_name)
        {
            return;
        }

        let mut enum_value = EnumValue::new(graph_name.into());
        enum_value
            .directives
            .push(join_graph_directive(subgraph_name));
        enum_type.values.push(enum_value);
    }
}

fn ensure_join_type(directives: &mut Vec<Directive<'static, String>>, graph_name: &str) {
    let already_present = directives.iter().any(|directive| {
        if directive.name.as_str() != "join__type" {
            return false;
        }
        directive.arguments.iter().any(|(name, value)| {
            name.as_str() == "graph"
                && matches!(value, Value::Enum(existing) if existing == graph_name)
        })
    });

    if already_present {
        return;
    }

    let mut directive = new_directive("join__type");
    directive
        .arguments
        .push(("graph".into(), Value::Enum(graph_name.into())));
    directives.insert(0, directive);
}

fn ensure_join_field(directives: &mut Vec<Directive<'static, String>>, graph_name: &str) {
    if directives
        .iter()
        .any(|directive| directive.name.as_str() == "join__field")
    {
        return;
    }

    let mut directive = new_directive("join__field");
    directive
        .arguments
        .push(("graph".into(), Value::Enum(graph_name.into())));
    directives.insert(0, directive);
}

fn ensure_join_enum_value(directives: &mut Vec<Directive<'static, String>>, graph_name: &str) {
    if directives
        .iter()
        .any(|directive| directive.name.as_str() == "join__enumValue")
    {
        return;
    }

    let mut directive = new_directive("join__enumValue");
    directive
        .arguments
        .push(("graph".into(), Value::Enum(graph_name.into())));
    directives.insert(0, directive);
}

fn ensure_join_union_members(
    directives: &mut Vec<Directive<'static, String>>,
    members: &[String],
    graph_name: &str,
) {
    for member in members {
        let already_present = directives.iter().any(|directive| {
            directive.name.as_str() == "join__unionMember"
                && directive.arguments.iter().any(|(name, value)| {
                    name.as_str() == "member"
                        && matches!(value, Value::String(existing) if existing == member)
                })
        });

        if already_present {
            continue;
        }

        let mut directive = new_directive("join__unionMember");
        directive
            .arguments
            .push(("graph".into(), Value::Enum(graph_name.into())));
        directive
            .arguments
            .push(("member".into(), Value::String(member.clone())));
        directives.push(directive);
    }
}

fn join_graph_directive(subgraph_name: &str) -> Directive<'static, String> {
    let mut directive = new_directive("join__graph");
    directive
        .arguments
        .push(("name".into(), Value::String(subgraph_name.into())));
    directive.arguments.push((
        "url".into(),
        Value::String(format!("extension://{}", subgraph_name)),
    ));
    directive
}

fn find_object_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut ObjectType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Object(object))
                if object.name.as_str() == name =>
            {
                Some(object)
            }
            _ => None,
        })
}

fn find_interface_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut InterfaceType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Interface(interface))
                if interface.name.as_str() == name =>
            {
                Some(interface)
            }
            _ => None,
        })
}

fn find_input_object_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut InputObjectType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::InputObject(input))
                if input.name.as_str() == name =>
            {
                Some(input)
            }
            _ => None,
        })
}

fn find_enum_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut EnumType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Enum(enum_type))
                if enum_type.name.as_str() == name =>
            {
                Some(enum_type)
            }
            _ => None,
        })
}

fn find_union_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut UnionType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Union(union_type))
                if union_type.name.as_str() == name =>
            {
                Some(union_type)
            }
            _ => None,
        })
}

fn find_scalar_type_mut<'a>(
    document: &'a mut Document<'static, String>,
    name: &str,
) -> Option<&'a mut ScalarType<'static, String>> {
    document
        .definitions
        .iter_mut()
        .find_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Scalar(scalar))
                if scalar.name.as_str() == name =>
            {
                Some(scalar)
            }
            _ => None,
        })
}

fn new_directive(name: &str) -> Directive<'static, String> {
    Directive {
        position: Pos::default(),
        name: name.into(),
        arguments: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_supergraph(sdl: &str) -> Document<'static, String> {
        graphql_parser::parse_schema::<String>(sdl)
            .expect("supergraph SDL should parse")
            .into_static()
    }

    #[test]
    fn composes_extension_fields_and_types() {
        let mut composer = SchemaComposer::new();
        composer
            .add_subgraph(
                "issues".into(),
                r#"
extend type Query {
  issues(path: String!): [Issue!]!
}

type Issue {
  id: ID!
  title: String!
}

enum IssueState {
  OPEN
  CLOSED
}
"#
                .into(),
            )
            .expect("extension SDL should parse");

        let supergraph_sdl = composer.compose().expect("composition should succeed");
        let document = parse_supergraph(&supergraph_sdl);

        let query = find_object_type(&document, "Query").expect("query type exists");
        let issues_field = query
            .fields
            .iter()
            .find(|field| field.name == "issues")
            .expect("issues field present");
        assert!(has_directive(&issues_field.directives, "join__field"));

        let issue_type = find_object_type(&document, "Issue").expect("issue type exists");
        assert!(has_directive(&issue_type.directives, "join__type"));
        for field in &issue_type.fields {
            assert!(has_directive(&field.directives, "join__field"));
        }

        let issue_state = find_enum_type(&document, "IssueState").expect("enum exists");
        assert!(has_directive(&issue_state.directives, "join__type"));
        for value in &issue_state.values {
            assert!(has_directive(&value.directives, "join__enumValue"));
        }

        let join_graph = find_enum_type(&document, "join__Graph").expect("join enum exists");
        let issues_graph = join_graph
            .values
            .iter()
            .find(|value| value.name == "ISSUES")
            .expect("enum value registered");
        let join_graph_directive = issues_graph
            .directives
            .iter()
            .find(|directive| directive.name == "join__graph")
            .expect("join graph directive present");
        let mut arguments = join_graph_directive.arguments.clone();
        arguments.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0].0, "name");
        assert_eq!(arguments[0].1, Value::String("issues".into()));
        assert_eq!(arguments[1].0, "url");
        assert_eq!(arguments[1].1, Value::String("extension://issues".into()));
    }

    fn find_object_type<'a>(
        document: &'a Document<'static, String>,
        name: &str,
    ) -> Option<&'a ObjectType<'static, String>> {
        document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::TypeDefinition(TypeDefinition::Object(object))
                    if object.name.as_str() == name =>
                {
                    Some(object)
                }
                _ => None,
            })
    }

    fn find_enum_type<'a>(
        document: &'a Document<'static, String>,
        name: &str,
    ) -> Option<&'a EnumType<'static, String>> {
        document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::TypeDefinition(TypeDefinition::Enum(enum_type))
                    if enum_type.name.as_str() == name =>
                {
                    Some(enum_type)
                }
                _ => None,
            })
    }

    fn has_directive(directives: &[Directive<'static, String>], name: &str) -> bool {
        directives
            .iter()
            .any(|directive| directive.name.as_str() == name)
    }
}
