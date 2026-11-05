//! Revisão geométrica por limites explícitos; não certifica processos de fabricação.
use crate::PcbDiagnostic;
use eda_core::{ComponentId, NmPoint, PadShape, PadTechnology, Project};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManufacturingLimits {
    #[serde(default = "default_drill_clearance_nm")]
    pub min_drill_clearance_nm: i64,
    pub min_track_width_nm: i64,
    pub min_clearance_nm: i64,
    pub min_via_drill_nm: i64,
    pub min_pth_drill_nm: i64,
    pub min_npth_drill_nm: i64,
    pub min_annular_ring_nm: i64,
    pub min_copper_edge_clearance_nm: i64,
}
fn default_drill_clearance_nm() -> i64 {
    250_000
}
impl Default for ManufacturingLimits {
    fn default() -> Self {
        Self {
            min_drill_clearance_nm: default_drill_clearance_nm(),
            min_track_width_nm: 150_000,
            min_clearance_nm: 150_000,
            min_via_drill_nm: 300_000,
            min_pth_drill_nm: 300_000,
            min_npth_drill_nm: 500_000,
            min_annular_ring_nm: 100_000,
            min_copper_edge_clearance_nm: 250_000,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ManufacturingDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub components: Vec<ComponentId>,
    pub position: Option<NmPoint>,
    pub object_kind: Option<String>,
    pub object_id: Option<u64>,
    pub pad_number: Option<String>,
    pub actual_nm: Option<i64>,
    pub minimum_nm: Option<i64>,
}
impl From<PcbDiagnostic> for ManufacturingDiagnostic {
    fn from(value: PcbDiagnostic) -> Self {
        Self {
            code: value.code,
            severity: value.severity,
            message: value.message,
            components: value.components,
            position: value.position,
            object_kind: None,
            object_id: None,
            pad_number: None,
            actual_nm: None,
            minimum_nm: None,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ManufacturingReport {
    pub limits: ManufacturingLimits,
    pub diagnostics: Vec<ManufacturingDiagnostic>,
    pub complete: bool,
}
impl ManufacturingReport {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count()
    }
}
pub fn check(
    project: &Project,
    limits: &ManufacturingLimits,
) -> Result<ManufacturingReport, String> {
    let values = [
        limits.min_drill_clearance_nm,
        limits.min_track_width_nm,
        limits.min_clearance_nm,
        limits.min_via_drill_nm,
        limits.min_pth_drill_nm,
        limits.min_npth_drill_nm,
        limits.min_annular_ring_nm,
        limits.min_copper_edge_clearance_nm,
    ];
    if values.iter().any(|v| !(0..=50_000_000).contains(v))
        || limits.min_track_width_nm == 0
        || limits.min_via_drill_nm == 0
        || limits.min_pth_drill_nm == 0
        || limits.min_npth_drill_nm == 0
    {
        return Err(eda_i18n::text("Limites de fabricação inválidos."));
    }
    project.validate().map_err(|e| e.to_string())?;
    let mut snapshot = project.clone();
    let board = snapshot
        .board
        .as_mut()
        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
    board.clearance_nm = board.clearance_nm.max(limits.min_clearance_nm);
    board.edge_clearance_nm = board
        .edge_clearance_nm
        .max(limits.min_copper_edge_clearance_nm);
    let mut diagnostics = Vec::new();
    let mut complete = true;
    let mut add = |code: &str,
                   position: NmPoint,
                   kind: &str,
                   id: Option<u64>,
                   component: Option<ComponentId>,
                   pad_number: Option<String>,
                   actual: i64,
                   minimum: i64| {
        diagnostics.push(ManufacturingDiagnostic {
            code: code.into(),
            severity: "error".into(),
            message: eda_i18n::text(code),
            components: component.into_iter().collect(),
            position: Some(position),
            object_kind: Some(kind.into()),
            object_id: id,
            pad_number,
            actual_nm: Some(actual),
            minimum_nm: Some(minimum),
        });
    };
    for track in board.tracks.values() {
        if track.width_nm < limits.min_track_width_nm {
            add(
                "DFM_TRACK_WIDTH",
                track.start,
                "track",
                Some(track.id),
                None,
                None,
                track.width_nm,
                limits.min_track_width_nm,
            );
        }
    }
    for via in board.vias.values() {
        if via.drill_nm < limits.min_via_drill_nm {
            add(
                "DFM_VIA_DRILL",
                via.position,
                "via",
                Some(via.id),
                None,
                None,
                via.drill_nm,
                limits.min_via_drill_nm,
            );
        }
        let ring = (via.diameter_nm - via.drill_nm) / 2;
        if ring < limits.min_annular_ring_nm {
            add(
                "DFM_ANNULAR_RING",
                via.position,
                "via",
                Some(via.id),
                None,
                None,
                ring,
                limits.min_annular_ring_nm,
            );
        }
    }
    for hole in board.holes.values() {
        if hole.drill_nm < limits.min_npth_drill_nm {
            add(
                "DFM_NPTH_DRILL",
                hole.position,
                "hole",
                Some(hole.id),
                None,
                None,
                hole.drill_nm,
                limits.min_npth_drill_nm,
            );
        }
    }
    let pads = crate::placed_pads(project)?;
    for pad in &pads {
        if let Some(drill) = pad.pad.drill {
            let minimum = if pad.pad.technology == PadTechnology::NonPlated {
                limits.min_npth_drill_nm
            } else {
                limits.min_pth_drill_nm
            };
            let tool = drill.x.min(drill.y);
            if tool < minimum {
                add(
                    "DFM_PAD_DRILL",
                    pad.position,
                    "pad",
                    None,
                    Some(pad.component),
                    Some(pad.number.clone()),
                    tool,
                    minimum,
                );
            }
            let ring = (pad.pad.size.x - drill.x).min(pad.pad.size.y - drill.y) / 2;
            if pad.pad.technology != PadTechnology::NonPlated && ring < limits.min_annular_ring_nm {
                add(
                    "DFM_ANNULAR_RING",
                    pad.position,
                    "pad",
                    None,
                    Some(pad.component),
                    Some(pad.number.clone()),
                    ring,
                    limits.min_annular_ring_nm,
                );
            }
        }
    }
    for pad in &pads {
        if pad.pad.drill.is_some()
            && pad.pad.technology != PadTechnology::NonPlated
            && (pad.pad.shape == PadShape::Custom || !pad.pad.polygons.is_empty())
        {
            complete = false;
            diagnostics.push(ManufacturingDiagnostic {
                code: "DFM_CUSTOM_PAD_REVIEW".into(),
                severity: "warning".into(),
                message: eda_i18n::text("DFM_CUSTOM_PAD_REVIEW"),
                components: vec![pad.component],
                position: Some(pad.position),
                object_kind: Some("pad".into()),
                object_id: None,
                pad_number: Some(pad.number.clone()),
                actual_nm: None,
                minimum_nm: None,
            });
        }
    }
    let drill_report =
        crate::drilling::check_drill_clearance(project, &pads, limits.min_drill_clearance_nm)?;
    if drill_report.len() >= 1000 {
        complete = false;
    }
    diagnostics.extend(drill_report.into_iter().map(|mut diagnostic| {
        diagnostic.code = "DFM_DRILL_CLEARANCE".into();
        diagnostic.message = format!(
            "{} {}",
            eda_i18n::text("DFM_DRILL_CLEARANCE"),
            diagnostic.message
        );
        ManufacturingDiagnostic::from(diagnostic)
    }));
    let native = crate::check(&snapshot)?;
    if native.diagnostics.len() >= 2000 {
        complete = false;
    }
    diagnostics.extend(
        native
            .diagnostics
            .into_iter()
            .map(ManufacturingDiagnostic::from),
    );
    Ok(ManufacturingReport {
        limits: limits.clone(),
        diagnostics,
        complete,
    })
}
