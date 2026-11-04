use eda_core::Project;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeReport {
    pub format: String,
    pub electrical_complete: bool,
    pub geometry_complete: bool,
    pub losses: Vec<InterchangeIssue>,
    pub warnings: Vec<String>,
}

impl InterchangeReport {
    pub fn lossless(&self) -> bool {
        self.electrical_complete && self.geometry_complete && self.losses.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct InterchangeResult {
    pub project: Project,
    pub report: InterchangeReport,
}
