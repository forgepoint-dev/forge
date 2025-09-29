//! Dynamic schema management for extensions with basic conflict detection

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use super::Extension;

/// Schema manager handles merging extension schemas with basic conflict detection
pub struct SchemaManager {
    extension_schemas: HashMap<String, String>,
    type_registry: HashMap<String, String>, // type_name -> extension_name
    field_registry: HashMap<String, HashSet<String>>, // type_name -> set of field names
}

impl SchemaManager {
    pub fn new() -> Self {
        Self {
            extension_schemas: HashMap::new(),
            type_registry: HashMap::new(),
            field_registry: HashMap::new(),
        }
    }

    /// Parse and validate an extension's GraphQL schema SDL with basic conflict detection
    pub fn parse_extension_schema(&mut self, extension: &Extension) -> Result<()> {
        if extension.schema_sdl.is_empty() {
            return Ok(());
        }

        // Basic validation and conflict detection using string parsing
        self.validate_extension_schema(&extension.name, &extension.schema_sdl)?;
        self.detect_basic_conflicts(&extension.name, &extension.schema_sdl)?;

        // Store the SDL
        self.extension_schemas.insert(extension.name.clone(), extension.schema_sdl.clone());

        tracing::info!("Successfully registered schema for extension: {}", extension.name);
        Ok(())
    }

    /// Validate basic schema structure and compliance rules
    fn validate_extension_schema(&self, extension_name: &str, schema_sdl: &str) -> Result<()> {
        if schema_sdl.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Extension '{}' provided empty schema", 
                extension_name
            ));
        }

        // Check for direct root type definitions (not allowed)
        let lines: Vec<&str> = schema_sdl.lines().collect();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with("type Query") || 
               trimmed.starts_with("type Mutation") || 
               trimmed.starts_with("type Subscription") {
                return Err(anyhow::anyhow!(
                    "Extension '{}' cannot define root types Query, Mutation, or Subscription. Use 'extend type' instead.",
                    extension_name
                ));
            }
            
            // Check for schema definition (not allowed)
            if trimmed.starts_with("schema") {
                return Err(anyhow::anyhow!(
                    "Extension '{}' cannot define schema directive. This is reserved for the host.",
                    extension_name
                ));
            }
        }

        Ok(())
    }

    /// Basic conflict detection using string parsing
    fn detect_basic_conflicts(&mut self, extension_name: &str, schema_sdl: &str) -> Result<()> {
        let lines: Vec<&str> = schema_sdl.lines().collect();
        
        for line in lines {
            let trimmed = line.trim();
            
            // Look for type definitions
            if trimmed.starts_with("type ") && !trimmed.starts_with("type Query") 
                && !trimmed.starts_with("type Mutation") && !trimmed.starts_with("type Subscription") {
                if let Some(type_name) = extract_type_name(trimmed) {
                    // Check for type name conflicts
                    if let Some(existing_extension) = self.type_registry.get(&type_name) {
                        if existing_extension != extension_name {
                            return Err(anyhow::anyhow!(
                                "Type name conflict: '{}' is already defined by extension '{}', cannot be redefined by '{}'",
                                type_name, existing_extension, extension_name
                            ));
                        }
                    }
                    self.type_registry.insert(type_name, extension_name.to_string());
                }
            }
        }

        Ok(())
    }

    /// Get all extension schema SDLs
    pub fn get_extension_schemas(&self) -> &HashMap<String, String> {
        &self.extension_schemas
    }

    /// Create a merged schema SDL string for all extensions
    pub fn create_merged_schema_sdl(&self) -> String {
        let mut merged = String::new();
        
        // Add schema metadata
        merged.push_str("# Merged GraphQL schema from extensions\n");
        merged.push_str("# Generated automatically - do not edit\n\n");
        
        // Concatenate all extension schemas with validation
        for (extension_name, schema_sdl) in &self.extension_schemas {
            merged.push_str(&format!("# Schema from {} extension\n", extension_name));
            merged.push_str(schema_sdl);
            merged.push_str("\n\n");
        }
        
        merged
    }

    /// Get schema for a specific extension
    pub fn get_extension_schema(&self, extension_name: &str) -> Option<&String> {
        self.extension_schemas.get(extension_name)
    }

    /// Check if any extensions are loaded
    pub fn has_extensions(&self) -> bool {
        !self.extension_schemas.is_empty()
    }

    /// Get conflict summary for debugging
    pub fn get_conflict_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("Schema Registry Summary:\n");
        summary.push_str(&format!("Total extensions: {}\n", self.extension_schemas.len()));
        summary.push_str(&format!("Total types: {}\n", self.type_registry.len()));
        
        for (type_name, extension) in &self.type_registry {
            summary.push_str(&format!("  {} ({})\n", type_name, extension));
        }
        
        summary
    }
}

/// Extract type name from a type definition line
fn extract_type_name(line: &str) -> Option<String> {
    // Simple parsing: "type TypeName {" -> "TypeName"
    if let Some(start) = line.find("type ") {
        let after_type = &line[start + 5..];
        if let Some(end) = after_type.find(&[' ', '{', '\t'][..]) {
            let type_name = after_type[..end].trim();
            if !type_name.is_empty() {
                return Some(type_name.to_string());
            }
        }
    }
    None
}