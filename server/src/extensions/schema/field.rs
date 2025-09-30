use super::sdl::{format_description, indent_spaces};
use super::types::{InputValueDefinition, TypeRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub description: Option<String>,
    pub ty: TypeRef,
    pub args: Vec<InputValueDefinition>,
}

impl FieldDefinition {
    #[allow(dead_code)]
    pub fn to_sdl(&self, indent: usize) -> String {
        let mut output = String::new();
        let prefix = indent_spaces(indent);

        if let Some(formatted_desc) = format_description(self.description.as_deref(), indent) {
            output.push_str(&formatted_desc);
        }

        output.push_str(&prefix);
        output.push_str(&self.name);

        if !self.args.is_empty() {
            output.push('(');
            let arg_strings: Vec<String> = self
                .args
                .iter()
                .map(|arg| format!("{}: {}", arg.name, arg.ty.to_sdl()))
                .collect();
            output.push_str(&arg_strings.join(", "));
            output.push(')');
        }

        output.push_str(": ");
        output.push_str(&self.ty.to_sdl());
        output
    }
}
