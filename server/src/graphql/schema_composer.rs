use anyhow::Result;
use std::collections::HashMap;

/// Simple schema composition utility
/// In a full implementation, this would use a proper federation composition library
/// For now, we'll create a basic supergraph manually
pub struct SchemaComposer {
    subgraphs: HashMap<String, String>,
    core_schema: String,
}

impl SchemaComposer {
    pub fn new() -> Self {
        // Start with a basic core schema
        let core_schema = r#"
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
  browseRepository(path: String!, treePath: String): RepositoryEntriesPayload @join__field(graph: CORE)
}

type Mutation @join__type(graph: CORE) {
  # Core mutations
  createGroup(input: CreateGroupInput!): GroupNode! @join__field(graph: CORE)
  createRepository(input: CreateRepositoryInput!): RepositoryNode! @join__field(graph: CORE)
  linkRemoteRepository(url: String!): RepositoryNode! @join__field(graph: CORE)
}

# Core types
type GroupNode @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
}

type RepositoryNode @join__type(graph: CORE) {
  id: ID! @join__field(graph: CORE)
  slug: String! @join__field(graph: CORE)
  isRemote: Boolean! @join__field(graph: CORE)
  remoteUrl: String @join__field(graph: CORE)
}

type RepositoryEntriesPayload @join__type(graph: CORE) {
  treePath: String @join__field(graph: CORE)
  entries: [RepositoryEntry!]! @join__field(graph: CORE)
}

type RepositoryEntry @join__type(graph: CORE) {
  name: String! @join__field(graph: CORE)
  path: String! @join__field(graph: CORE)
  type: EntryType! @join__field(graph: CORE)
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
"#.to_string();

        Self {
            subgraphs: HashMap::new(),
            core_schema,
        }
    }

    pub fn add_subgraph(&mut self, name: String, schema: String) {
        self.subgraphs.insert(name, schema);
    }

    pub fn compose(&self) -> Result<String> {
        let mut supergraph = self.core_schema.clone();

        // Add subgraph entries to join__Graph enum
        let mut graph_enum_additions = Vec::new();

        for (name, _schema) in &self.subgraphs {
            graph_enum_additions.push(format!(
                "  {} @join__graph(name: \"{}\", url: \"internal://{}\")",
                name.to_uppercase(),
                name,
                name
            ));
        }

        if !graph_enum_additions.is_empty() {
            // Insert additional graphs into the enum
            supergraph = supergraph.replace(
                "enum join__Graph {\n  CORE @join__graph(name: \"core\", url: \"internal://core\")\n}",
                &format!(
                    "enum join__Graph {{\n  CORE @join__graph(name: \"core\", url: \"internal://core\")\n{}\n}}",
                    graph_enum_additions.join("\n")
                )
            );
        }

        // Add extension types and fields
        for (name, schema) in &self.subgraphs {
            tracing::debug!("Processing extension '{}' with schema:\n{}", name, schema);
            let processed_schema = self.process_extension_schema(name, schema)?;
            tracing::debug!("Processed schema for '{}' :\n{}", name, processed_schema);
            supergraph.push_str("\n\n");
            supergraph.push_str(&processed_schema);
        }

        tracing::debug!("Final supergraph SDL:\n{}", supergraph);
        Ok(supergraph)
    }

    fn process_extension_schema(&self, subgraph_name: &str, schema: &str) -> Result<String> {
        // This is a simplified processing - in reality we'd parse the SDL properly
        // For now, we'll do basic string processing to add @join__ directives

        let graph_name = subgraph_name.to_uppercase();

        // Parse extension schema and add @join__ directives
        let mut processed = String::new();
        let mut inside_extend_block = false;
        let mut brace_depth = 0;

        for line in schema.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("#") {
                continue;
            }

            // Handle extend type blocks - we skip these entirely since we'll merge fields into core types
            if line.starts_with("extend type") {
                inside_extend_block = true;
                brace_depth = 0;
                continue;
            }

            if inside_extend_block {
                // Count braces to know when the extend block ends
                brace_depth += line.chars().filter(|&c| c == '{').count() as i32;
                brace_depth -= line.chars().filter(|&c| c == '}').count() as i32;

                if brace_depth <= 0 {
                    inside_extend_block = false;
                }
                continue; // Skip all content inside extend blocks
            }

            // Skip any orphaned field definitions (lines that contain ":" but are not within a proper type definition)
            if line.contains(":") && !line.contains("directive") && !line.contains("scalar") && !line.contains("enum") && !line.contains("input") && !line.contains("type") && !line.contains("interface") && !line.contains("union") {
                continue; // Skip orphaned field definitions
            }

            // Process regular schema definitions
            if line.starts_with("type ") && !line.contains("@join__type") {
                // Add join type directive to regular types, handling existing directives
                if line.contains("{") {
                    // Type definition with opening brace on same line
                    let parts: Vec<&str> = line.splitn(2, '{').collect();
                    processed.push_str(&format!("{} @join__type(graph: {}) {{\n", parts[0].trim(), graph_name));
                } else if line.contains("@") {
                    // Type already has directives, add ours before existing ones
                    let parts: Vec<&str> = line.splitn(2, '@').collect();
                    processed.push_str(&format!("{} @join__type(graph: {}) @{}\n", parts[0].trim(), graph_name, parts[1]));
                } else {
                    // No existing directives
                    processed.push_str(&format!("{} @join__type(graph: {})\n", line, graph_name));
                }
            } else if line.starts_with("enum ") && !line.contains("@join__type") {
                // Add join type directive to enums
                if line.contains("{") {
                    // Enum definition with opening brace on same line
                    let parts: Vec<&str> = line.splitn(2, '{').collect();
                    processed.push_str(&format!("{} @join__type(graph: {}) {{\n", parts[0].trim(), graph_name));
                } else {
                    processed.push_str(&format!("{} @join__type(graph: {})\n", line, graph_name));
                }
            } else if line.starts_with("input ") && !line.contains("@join__type") {
                // Add join type directive to input types
                if line.contains("{") {
                    // Input definition with opening brace on same line
                    let parts: Vec<&str> = line.splitn(2, '{').collect();
                    processed.push_str(&format!("{} @join__type(graph: {}) {{\n", parts[0].trim(), graph_name));
                } else {
                    processed.push_str(&format!("{} @join__type(graph: {})\n", line, graph_name));
                }
            } else if line.contains(":") && !line.contains("@join__field") {
                // Add join field directive to field definitions
                if line.contains("@") {
                    // Field already has directives, add ours before existing ones
                    let parts: Vec<&str> = line.splitn(2, '@').collect();
                    processed.push_str(&format!("  {} @join__field(graph: {}) @{}\n", parts[0].trim(), graph_name, parts[1]));
                } else {
                    // No existing directives
                    processed.push_str(&format!("  {} @join__field(graph: {})\n", line, graph_name));
                }
            } else {
                // Copy other lines as-is (opening/closing braces, etc.)
                processed.push_str(&format!("{}\n", line));
            }
        }

        Ok(processed)
    }
}

impl Default for SchemaComposer {
    fn default() -> Self {
        Self::new()
    }
}