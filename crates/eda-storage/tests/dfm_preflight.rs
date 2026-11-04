use eda_core::{Board, MountingHole, NmPoint, Project};
use eda_pcb::dfm::ManufacturingLimits;
use eda_storage::fabrication::{ManufacturingPreset, generate_with_preset_and_limits};
#[test]
fn dfm_preflight_blocks_before_tool_or_destination_side_effects() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("package");
    let tool = directory.path().join("nonexistent-tool.exe");
    let mut project = Project::empty("DFM");
    let mut board = Board::default();
    board.holes.insert(
        1,
        MountingHole {
            id: 1,
            position: NmPoint::new(10_000_000, 10_000_000),
            drill_nm: 400_000,
        },
    );
    project.board = Some(Box::new(board));
    let before = serde_json::to_value(&project).unwrap();
    let error = generate_with_preset_and_limits(
        &project,
        &destination,
        &tool,
        ManufacturingPreset::Standard,
        Some(&ManufacturingLimits::default()),
    )
    .unwrap_err();
    assert!(!error.contains("kicad-cli.exe"));
    assert!(!destination.exists());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    assert_eq!(before, serde_json::to_value(&project).unwrap());
}
