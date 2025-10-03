use super::field::FieldDefinition;
use anyhow::{Result, bail};
use std::collections::HashSet;

#[allow(dead_code)]
pub fn ensure_unique_field_names(
    fields: &[FieldDefinition],
    extension: &str,
    ty: &str,
) -> Result<()> {
    let field_names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
    ensure_unique_names(field_names.into_iter(), extension, ty, "field")
}

#[allow(dead_code)]
pub fn ensure_unique_names<'a, I>(names: I, extension: &str, ty: &str, scope: &str) -> Result<()>
where
    I: Iterator<Item = &'a str>,
{
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name) {
            bail!(
                "Duplicate {} name '{}' in {} '{}' from extension '{}'",
                scope,
                name,
                ty,
                ty,
                extension
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "validator_tests.rs"]
mod tests;

#[allow(dead_code)]
pub fn is_root_type(name: &str) -> bool {
    matches!(name, "Query" | "Mutation" | "Subscription")
}
