//! Variantes de montagem derivadas sem modificar o projeto elétrico ou a PCB.
use crate::assembly::{AssemblyIssue, AssemblyReport, assembly_report};
use eda_core::{ComponentKind, Project};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyVariant {
    pub name: String,
    #[serde(default)]
    pub do_not_populate: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VariantAssemblyReport {
    pub variant: String,
    pub omitted: Vec<AssemblyIssue>,
    pub assembly: AssemblyReport,
}

pub fn variant_assembly_report(
    project: &Project,
    variant: &AssemblyVariant,
) -> Result<VariantAssemblyReport, String> {
    project.validate().map_err(|e| e.to_string())?;
    if variant.name.trim().is_empty()
        || variant.name.len() > 128
        || variant.name.chars().any(char::is_control)
        || variant.do_not_populate.len() > 4096
    {
        return Err(eda_i18n::text("assembly_variant.invalid_config"));
    }
    let mut selected = BTreeSet::new();
    let mut omitted = Vec::new();
    for reference in &variant.do_not_populate {
        if !selected.insert(reference) {
            return Err(eda_i18n::text("assembly_variant.duplicate_reference"));
        }
        let component = project
            .components
            .values()
            .find(|c| &c.reference == reference)
            .filter(|c| c.kind != ComponentKind::Ground)
            .ok_or_else(|| eda_i18n::text("assembly_variant.invalid_reference"))?;
        omitted.push(AssemblyIssue {
            component_id: component.id.0,
            reference: reference.clone(),
        });
    }
    omitted.sort_by(|a, b| a.reference.cmp(&b.reference));
    let mut assembly = assembly_report(project);
    for row in &mut assembly.bom {
        row.references.retain(|r| !selected.contains(r));
        row.quantity = row.references.len();
    }
    assembly.bom.retain(|r| r.quantity > 0);
    assembly
        .placements
        .retain(|r| !selected.contains(&r.reference));
    assembly
        .missing_footprint
        .retain(|r| !selected.contains(&r.reference));
    assembly
        .unplaced
        .retain(|r| !selected.contains(&r.reference));
    Ok(VariantAssemblyReport {
        variant: variant.name.clone(),
        omitted,
        assembly,
    })
}
