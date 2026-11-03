//! Relatórios de montagem derivados do projeto, sem alterar seus dados.
//!
//! Posições estão em nanômetros no referencial da placa. Rotação em graus é
//! `Placement.rotation * 90`, anti-horária no referencial do domínio; a face
//! inferior é espelhada antes da rotação. Fabricantes podem exigir outra convenção.

use eda_core::{BoardSide, ComponentKind, Project};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct AssemblyIssue {
    pub component_id: u64,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BomRow {
    pub references: Vec<String>,
    pub quantity: usize,
    pub kind: String,
    pub value: String,
    pub unit: String,
    pub device: String,
    pub footprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementRow {
    pub component_id: u64,
    pub reference: String,
    pub kind: String,
    pub value: String,
    pub unit: String,
    pub device: String,
    pub footprint: String,
    pub side: &'static str,
    pub x_nm: i64,
    pub y_nm: i64,
    pub rotation_deg: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssemblyReport {
    pub bom: Vec<BomRow>,
    pub placements: Vec<PlacementRow>,
    pub missing_footprint: Vec<AssemblyIssue>,
    pub unplaced: Vec<AssemblyIssue>,
}

/// Inclui todos os componentes exceto terra, inclusive os incompletos na BOM.
pub fn assembly_report(project: &Project) -> AssemblyReport {
    let mut groups: BTreeMap<(String, String, String, String, String), Vec<String>> =
        BTreeMap::new();
    let mut placements = Vec::new();
    let mut missing_footprint = Vec::new();
    let mut unplaced = Vec::new();
    for component in project
        .components
        .values()
        .filter(|c| c.kind != ComponentKind::Ground)
    {
        let kind = format!("{:?}", component.kind);
        let value = component
            .imported
            .as_ref()
            .map_or_else(|| component.value.to_string(), |i| i.value_text.clone());
        let unit = if component.imported.is_some() {
            String::new()
        } else {
            component.kind.unit().to_owned()
        };
        let device = component
            .device
            .as_ref()
            .map(|d| d.id.clone())
            .or_else(|| component.imported.as_ref().map(|i| i.device.clone()))
            .unwrap_or_default();
        let footprint = component
            .footprint
            .as_ref()
            .map_or_else(String::new, |f| f.id.clone());
        groups
            .entry((
                kind.clone(),
                value.clone(),
                unit.clone(),
                device.clone(),
                footprint.clone(),
            ))
            .or_default()
            .push(component.reference.clone());
        let issue = || AssemblyIssue {
            component_id: component.id.0,
            reference: component.reference.clone(),
        };
        if component.footprint.is_none() {
            missing_footprint.push(issue());
        }
        let placement = project
            .board
            .as_ref()
            .and_then(|b| b.placements.get(&component.id));
        if placement.is_none() {
            unplaced.push(issue());
        }
        if let (Some(_), Some(p)) = (&component.footprint, placement) {
            placements.push(PlacementRow {
                component_id: component.id.0,
                reference: component.reference.clone(),
                kind,
                value,
                unit,
                device,
                footprint,
                side: match p.side {
                    BoardSide::Top => "top",
                    BoardSide::Bottom => "bottom",
                },
                x_nm: p.position.x,
                y_nm: p.position.y,
                rotation_deg: u16::from(p.rotation) * 90,
            });
        }
    }
    placements.sort_by(|a, b| {
        a.reference
            .cmp(&b.reference)
            .then(a.component_id.cmp(&b.component_id))
    });
    missing_footprint.sort_by(|a, b| {
        a.reference
            .cmp(&b.reference)
            .then(a.component_id.cmp(&b.component_id))
    });
    unplaced.sort_by(|a, b| {
        a.reference
            .cmp(&b.reference)
            .then(a.component_id.cmp(&b.component_id))
    });
    let bom = groups
        .into_iter()
        .map(|((kind, value, unit, device, footprint), mut references)| {
            references.sort();
            BomRow {
                quantity: references.len(),
                references,
                kind,
                value,
                unit,
                device,
                footprint,
            }
        })
        .collect();
    AssemblyReport {
        bom,
        placements,
        missing_footprint,
        unplaced,
    }
}

fn field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// CSV com campos textuais delimitados; não converte convenções de fabricantes.
pub fn bom_csv(report: &AssemblyReport) -> String {
    let mut out = String::from("references,quantity,kind,value,unit,device,footprint\r\n");
    for row in &report.bom {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\r\n",
            field(&row.references.join(";")),
            row.quantity,
            field(&row.kind),
            field(&row.value),
            field(&row.unit),
            field(&row.device),
            field(&row.footprint)
        ));
    }
    out
}

/// CSV com posições inteiras em nm e ângulo na convenção do domínio.
pub fn placements_csv(report: &AssemblyReport) -> String {
    let mut out =
        String::from("reference,kind,value,unit,device,footprint,side,x_nm,y_nm,rotation_deg\r\n");
    for row in &report.placements {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\r\n",
            field(&row.reference),
            field(&row.kind),
            field(&row.value),
            field(&row.unit),
            field(&row.device),
            field(&row.footprint),
            row.side,
            row.x_nm,
            row.y_nm,
            row.rotation_deg
        ));
    }
    out
}
