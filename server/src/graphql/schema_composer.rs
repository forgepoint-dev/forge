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

type RepositoryEntry @join__type(graph: CORE) {
  name: String! @join__field(graph: CORE)
  path: String! @join__field(graph: CORE)
  type: EntryType! @join__field(graph: CORE)
  size: Int @join__field(graph: CORE)
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

        // Collect extension fields to merge into Query/Mutation
        let mut query_extensions = Vec::new();
        let mut mutation_extensions = Vec::new();
        let mut types_to_add = Vec::new();
        let mut graph_entries: Vec<(String, String)> = Vec::new();

        // Add extension types and fields
        for (name, schema) in &self.subgraphs {
            tracing::debug!("Processing extension '{}' with schema:\n{}", name, schema);
            let (types, query_fields, mutation_fields, graph_name) =
                self.process_extension_schema(name, schema)?;
            tracing::debug!("Processed schema for '{}'", name);

            types_to_add.push(types);
            query_extensions.extend(query_fields);
            mutation_extensions.extend(mutation_fields);
            graph_entries.push((graph_name, name.clone()));
        }

        // Merge query extensions into the Query type
        if !query_extensions.is_empty() {
            let query_additions = query_extensions.join("\n  ");
            supergraph = supergraph.replace(
                "type Query @join__type(graph: CORE) {\n  # Core fields",
                &format!("type Query @join__type(graph: CORE) {{\n  # Core fields"),
            );
            supergraph = supergraph.replace(
                "  browseRepository(path: String!, treePath: String): RepositoryEntriesPayload @join__field(graph: CORE)\n}",
                &format!("  browseRepository(path: String!, treePath: String): RepositoryEntriesPayload @join__field(graph: CORE)\n  # Extension fields\n  {}\n}}", query_additions)
            );
        }

        // Merge mutation extensions into the Mutation type
        if !mutation_extensions.is_empty() {
            let mutation_additions = mutation_extensions.join("\n  ");
            supergraph = supergraph.replace(
                "type Mutation @join__type(graph: CORE) {\n  # Core mutations",
                &format!("type Mutation @join__type(graph: CORE) {{\n  # Core mutations"),
            );
            supergraph = supergraph.replace(
                "  linkRemoteRepository(url: String!): RepositoryNode! @join__field(graph: CORE)\n}",
                &format!("  linkRemoteRepository(url: String!): RepositoryNode! @join__field(graph: CORE)\n  # Extension mutations\n  {}\n}}", mutation_additions)
            );
        }

        // Add extension types
        for types in types_to_add {
            supergraph.push_str("\n\n");
            supergraph.push_str(&types);
        }

        if !graph_entries.is_empty() {
            let mut join_lines = Vec::with_capacity(graph_entries.len() + 1);
            join_lines
                .push("  CORE @join__graph(name: \"core\", url: \"internal://core\")".to_string());
            for (graph_name, subgraph_name) in &graph_entries {
                join_lines.push(format!(
                    "  {graph} @join__graph(name: \"{name}\", url: \"extension://{name}\")",
                    graph = graph_name,
                    name = subgraph_name
                ));
            }

            let join_block = format!("enum join__Graph {{\n{}\n}}\n", join_lines.join("\n"));

            supergraph = supergraph.replace(
                "enum join__Graph {\n  CORE @join__graph(name: \"core\", url: \"internal://core\")\n}\n",
                &join_block,
            );
        }

        tracing::debug!("Final supergraph SDL:\n{}", supergraph);

        // Temporary: write to file for debugging
        std::fs::write("/tmp/supergraph.graphql", &supergraph).ok();

        Ok(supergraph)
    }

    fn process_extension_schema(
        &self,
        subgraph_name: &str,
        schema: &str,
    ) -> Result<(String, Vec<String>, Vec<String>, String)> {
        // Returns: (types_string, query_fields, mutation_fields)

        let graph_name = subgraph_name.to_ascii_uppercase();

        // Parse extension schema and add @join__ directives
        let mut types = String::new();
        let mut query_fields = Vec::new();
        let mut mutation_fields = Vec::new();
        let mut inside_extend_block = false;
        let mut extend_type_name = String::new();
        let mut extend_fields = Vec::new();
        let mut inside_type_block = false;
        let mut current_type_lines = Vec::new();
        let mut type_brace_depth = 0;

        for line in schema.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("#") {
                continue;
            }

            // Handle extend type blocks - collect fields to be merged later
            if line.starts_with("extend type") {
                inside_extend_block = true;
                // Extract type name (e.g., "Query" from "extend type Query {")
                extend_type_name = line
                    .replace("extend type", "")
                    .replace("{", "")
                    .trim()
                    .to_string();
                extend_fields.clear();
                continue;
            }

            if inside_extend_block {
                // Skip opening brace
                if line == "{" {
                    continue;
                }

                // Check for closing brace
                if line == "}" {
                    inside_extend_block = false;
                    // Add the extended fields to appropriate list
                    if !extend_fields.is_empty() {
                        let fields_with_directive: Vec<String> = extend_fields
                            .iter()
                            .map(|f| format!("{} @join__field(graph: {})", f, graph_name))
                            .collect();

                        if extend_type_name == "Query" {
                            query_fields.extend(fields_with_directive);
                        } else if extend_type_name == "Mutation" {
                            mutation_fields.extend(fields_with_directive);
                        }
                    }
                    continue;
                }

                // Collect field definitions
                if line.contains(":") {
                    extend_fields.push(line.to_string());
                }
                continue;
            }

            // Handle regular type/enum/input definitions
            if (line.starts_with("type ")
                || line.starts_with("enum ")
                || line.starts_with("input "))
                && !inside_type_block
            {
                inside_type_block = true;
                type_brace_depth = 0;
                current_type_lines.clear();
                current_type_lines.push(line.to_string());

                // Check if opening brace is on the same line
                if line.contains("{") {
                    type_brace_depth += 1;
                }
                continue;
            }

            if inside_type_block {
                current_type_lines.push(line.to_string());

                // Count braces
                type_brace_depth += line.chars().filter(|&c| c == '{').count() as i32;
                type_brace_depth -= line.chars().filter(|&c| c == '}').count() as i32;

                // When we close the type definition, process it
                if type_brace_depth <= 0 {
                    inside_type_block = false;
                    // Process the complete type definition
                    let type_def = self.process_type_definition(&current_type_lines, &graph_name);
                    types.push_str(&type_def);
                    types.push('\n');
                }
                continue;
            }
        }

        Ok((types, query_fields, mutation_fields, graph_name))
    }

    fn process_type_definition(&self, lines: &[String], graph_name: &str) -> String {
        if lines.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        let first_line = &lines[0];

        // Add the type/enum/input declaration with @join__type directive
        if first_line.contains("@") {
            // Already has directives, add ours before existing ones
            let parts: Vec<&str> = first_line.splitn(2, '@').collect();
            result.push_str(&format!(
                "{} @join__type(graph: {}) @{}\n",
                parts[0].trim(),
                graph_name,
                parts[1]
            ));
        } else if first_line.contains("{") {
            // Opening brace on same line
            let parts: Vec<&str> = first_line.splitn(2, '{').collect();
            result.push_str(&format!(
                "{} @join__type(graph: {}) {{\n",
                parts[0].trim(),
                graph_name
            ));
        } else {
            // No directives or braces
            result.push_str(&format!(
                "{} @join__type(graph: {})\n",
                first_line, graph_name
            ));
        }

        // Process the body lines
        for line in lines.iter().skip(1) {
            if line == "{" || line == "}" {
                result.push_str(&format!("{}\n", line));
            } else if line.contains(":") && !line.contains("@join__field") {
                // This is a field definition - add @join__field directive
                if line.contains("@") {
                    // Already has directives, add ours before existing ones
                    let parts: Vec<&str> = line.splitn(2, '@').collect();
                    result.push_str(&format!(
                        "  {} @join__field(graph: {}) @{}\n",
                        parts[0].trim(),
                        graph_name,
                        parts[1]
                    ));
                } else {
                    result.push_str(&format!("  {} @join__field(graph: {})\n", line, graph_name));
                }
            } else {
                // Enum values or other content
                result.push_str(&format!("{}\n", line));
            }
        }

        result
    }
}

impl Default for SchemaComposer {
    fn default() -> Self {
        Self::new()
    }
}
