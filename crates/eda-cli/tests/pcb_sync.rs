use std::{path::Path, process::Command};

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn sync_adds_missing_placement_and_preserves_input_and_existing_board() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.protocraft");
    let output = dir.path().join("output.protocraft");
    assert!(
        cli(&["device-project", "STM32F103C8T6", "--output", path(&input)])
            .status
            .success()
    );
    let mut project = eda_storage::load(&input).unwrap();
    let board = project.board.as_mut().unwrap();
    board.placements.clear();
    eda_storage::save(&input, &project).unwrap();
    let original = std::fs::read(&input).unwrap();
    let report = cli(&["pcb-sync-report", path(&input)]);
    assert!(report.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["counts"]["unplaced"], 1);
    let result = cli(&["pcb-sync", path(&input), "--output", path(&output)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output_project = eda_storage::load(&output).unwrap();
    assert_eq!(output_project.board.as_ref().unwrap().placements.len(), 1);
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert_eq!(
        output_project.board.as_ref().unwrap().tracks,
        project.board.as_ref().unwrap().tracks
    );
    assert!(
        !cli(&["pcb-sync", path(&input), "--output", path(&output)])
            .status
            .success()
    );
    assert!(
        !cli(&["pcb-sync", path(&input), "--output", path(&input)])
            .status
            .success()
    );
    assert!(
        !cli(&[
            "pcb-sync",
            path(&input),
            "--output",
            path(&dir.path().join("bad.txt"))
        ])
        .status
        .success()
    );
    assert!(
        !cli(&["pcb-sync-report", path(&input), "extra"])
            .status
            .success()
    );
    let too_small = dir.path().join("small.protocraft");
    let failed_output = dir.path().join("failed.protocraft");
    let board = project.board.as_mut().unwrap();
    board.width_nm = eda_core::NM_PER_MM;
    board.height_nm = eda_core::NM_PER_MM;
    eda_storage::save(&too_small, &project).unwrap();
    let result = cli(&[
        "pcb-sync",
        path(&too_small),
        "--output",
        path(&failed_output),
    ]);
    assert!(!result.status.success());
    assert!(!failed_output.exists());
}

#[test]
fn report_preserves_positioned_board_and_copper() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.protocraft");
    let output = dir.path().join("output.protocraft");
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/555-led-1hz-pcb.protocraft");
    std::fs::copy(example, &input).unwrap();
    let before = eda_storage::load(&input).unwrap();
    let result = cli(&["pcb-sync", path(&input), "--output", path(&output)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let after = eda_storage::load(&output).unwrap();
    assert_eq!(before.board, after.board);
}
