use super::enum_type::EnumType;
use super::interface::InterfaceType;
use super::object::ObjectType;
use super::scalar::ScalarType;
use super::types::InputValueDefinition;
use std::fmt::Write;

#[allow(dead_code)]
pub fn render_object_like(object: &ObjectType, is_extension: bool) -> String {
    let mut output = String::new();

    if let Some(formatted_desc) = format_description(object.description.as_deref(), 0) {
        output.push_str(&formatted_desc);
    }

    if is_extension {
        output.push_str("extend ");
    }
    output.push_str("type ");

    if !object.interfaces.is_empty() {
        output.push_str(" implements ");
        output.push_str(&object.interfaces.join(" & "));
    }

    output.push_str(" {\n");

    for field in &object.fields {
        output.push_str(&field.to_sdl(2));
        output.push('\n');
    }

    output.push('}');
    output
}

#[allow(dead_code)]
pub fn render_interface(interface: &InterfaceType) -> String {
    let mut output = String::new();

    if let Some(formatted_desc) = format_description(interface.description.as_deref(), 0) {
        output.push_str(&formatted_desc);
    }

    output.push_str("interface ");
    output.push_str(" {\n");

    for field in &interface.fields {
        output.push_str(&field.to_sdl(2));
        output.push('\n');
    }

    output.push('}');
    output
}

#[allow(dead_code)]
pub fn render_enum(enum_type: &EnumType, name: &str) -> String {
    let mut output = String::new();

    if let Some(formatted_desc) = format_description(enum_type.description.as_deref(), 0) {
        output.push_str(&formatted_desc);
    }

    writeln!(&mut output, "enum {} {{", name).unwrap();

    for value in &enum_type.values {
        if let Some(formatted_desc) = format_description(value.description.as_deref(), 2) {
            output.push_str(&formatted_desc);
        }
        writeln!(&mut output, "  {}", value.name).unwrap();
    }

    write!(&mut output, "}}").unwrap();
    output
}

#[allow(dead_code)]
pub fn render_scalar(scalar: &ScalarType, name: &str) -> String {
    let mut output = String::new();

    if let Some(formatted_desc) = format_description(scalar.description.as_deref(), 0) {
        output.push_str(&formatted_desc);
    }

    write!(&mut output, "scalar {}", name).unwrap();
    output
}

#[allow(dead_code)]
pub fn render_union(types: &[String], name: &str) -> String {
    format!("union {} = {}", name, types.join(" | "))
}

#[allow(dead_code)]
pub fn render_input_object(fields: &[InputValueDefinition], name: &str) -> String {
    let mut output = String::new();

    writeln!(&mut output, "input {} {{", name).unwrap();

    for field in fields {
        if let Some(formatted_desc) = format_description(field.description.as_deref(), 2) {
            output.push_str(&formatted_desc);
        }
        writeln!(&mut output, "  {}: {}", field.name, field.ty.to_sdl()).unwrap();
    }

    write!(&mut output, "}}").unwrap();
    output
}

#[allow(dead_code)]
pub fn indent_spaces(indent: usize) -> String {
    " ".repeat(indent)
}

#[allow(dead_code)]
pub fn format_description(desc: Option<&str>, indent: usize) -> Option<String> {
    desc.map(|d| {
        let prefix = indent_spaces(indent);
        let trimmed = d.trim();

        if trimmed.contains('\n') {
            format!("{}\"\"\"\\n{}\\n{}\"\"\"\n", prefix, trimmed, prefix)
        } else {
            format!("{}\"{}\"\n", prefix, trimmed.replace('"', "\\\""))
        }
    })
}
