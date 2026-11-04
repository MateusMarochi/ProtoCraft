use eda_core::NmPoint;
use std::{
    path::Path,
    process::{Command, Output},
};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .env("PROTOCRAFT_LANGUAGE", "en")
        .args(args)
        .output()
        .unwrap()
}

fn save_example(path: &Path) -> eda_core::Project {
    let project = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    eda_storage::create_new(path, &project).unwrap();
    project
}

#[test]
fn dfm_profile_reports_object_and_limits_without_editing_or_overwriting() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("board.protocraft");
    let profile = directory.path().join("limits.json");
    let report_path = directory.path().join("review.json");
    save_example(&input);
    let original = std::fs::read(&input).unwrap();
    let default = cli(&["pcb-dfm", "--default-limits"]);
    assert!(default.status.success());
    let mut limits: serde_json::Value = serde_json::from_slice(&default.stdout).unwrap();
    limits["min_track_width_nm"] = 1_000_000.into();
    std::fs::write(&profile, serde_json::to_vec(&limits).unwrap()).unwrap();
    let args = [
        "pcb-dfm",
        input.to_str().unwrap(),
        "--limits",
        profile.to_str().unwrap(),
        "--output",
        report_path.to_str().unwrap(),
    ];
    let result = cli(&args);
    assert!(!result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report,
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&report_path).unwrap()).unwrap()
    );
    let violation = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "DFM_TRACK_WIDTH")
        .unwrap();
    assert_eq!(violation["minimum_nm"], 1_000_000);
    assert!(violation["actual_nm"].as_i64().unwrap() < 1_000_000);
    assert!(violation["object_id"].is_u64());
    assert!(
        !violation["message"]
            .as_str()
            .unwrap()
            .contains("DFM_TRACK_WIDTH")
    );
    assert_eq!(original, std::fs::read(&input).unwrap());
    let preserved = std::fs::read(&report_path).unwrap();
    assert!(!cli(&args).status.success());
    assert_eq!(preserved, std::fs::read(&report_path).unwrap());
    assert!(
        !cli(&[
            "pcb-dfm",
            input.to_str().unwrap(),
            "--limits",
            profile.to_str().unwrap(),
            "--limits",
            profile.to_str().unwrap()
        ])
        .status
        .success()
    );
    let manufacturing = directory.path().join("manufacturing");
    let blocked = cli(&[
        "pcb-manufacture",
        input.to_str().unwrap(),
        "--output",
        manufacturing.to_str().unwrap(),
        "--kicad",
        "unavailable-kicad-cli",
        "--dfm-limits",
        profile.to_str().unwrap(),
    ]);
    assert!(!blocked.status.success());
    let blocked_report: serde_json::Value = serde_json::from_slice(&blocked.stdout).unwrap();
    assert!(
        blocked_report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "DFM_TRACK_WIDTH")
    );
    assert!(
        !manufacturing.exists(),
        "DFM deve bloquear antes de chamar ferramenta ou criar pasta"
    );
}

#[test]
fn dfm_accepts_clean_board_and_rejects_untrusted_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("board.protocraft");
    let profile = directory.path().join("limits.json");
    save_example(&input);
    let valid = cli(&["pcb-dfm", input.to_str().unwrap()]);
    assert!(
        valid.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&valid.stdout).unwrap();
    assert_eq!(report["complete"], true);
    std::fs::write(&profile, b"{\"unknown\":1}").unwrap();
    assert!(
        !cli(&[
            "pcb-dfm",
            input.to_str().unwrap(),
            "--limits",
            profile.to_str().unwrap()
        ])
        .status
        .success()
    );
    std::fs::write(&profile, vec![b' '; 65_537]).unwrap();
    assert!(
        !cli(&[
            "pcb-dfm",
            input.to_str().unwrap(),
            "--limits",
            profile.to_str().unwrap()
        ])
        .status
        .success()
    );
    let help = cli(&["pcb-dfm", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
}

#[test]
fn cleanup_merges_split_track_through_cli_with_input_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("split.protocraft");
    let output = directory.path().join("merged.protocraft");
    let mut project = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    let board = project.board.as_mut().unwrap();
    let id = *board.tracks.keys().next().unwrap();
    let mut track = board.tracks[&id].clone();
    let end = track.end;
    let middle = NmPoint::new((track.start.x + end.x) / 2, (track.start.y + end.y) / 2);
    track.end = middle;
    board.tracks.insert(id, track.clone());
    track.id = board.tracks.keys().max().unwrap() + 1;
    track.start = middle;
    track.end = end;
    board.tracks.insert(track.id, track);
    let before_count = board.tracks.len();
    eda_storage::create_new(&input, &project).unwrap();
    let original = std::fs::read(&input).unwrap();
    let args = [
        "pcb-cleanup",
        input.to_str().unwrap(),
        "--merge-collinear",
        "--output",
        output.to_str().unwrap(),
    ];
    let result = cli(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["merged_tracks"].as_u64().unwrap() >= 1);
    let merged = eda_storage::load(&output).unwrap();
    assert_eq!(
        before_count - merged.board.as_ref().unwrap().tracks.len(),
        report["removed_tracks"].as_u64().unwrap() as usize
    );
    assert_eq!(0, eda_pcb::check(&merged).unwrap().errors());
    assert_eq!(original, std::fs::read(&input).unwrap());
    assert!(!cli(&args).status.success());
}

#[test]
fn routing_passes_are_available_in_cli_and_budget_is_global() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("unrouted.protocraft");
    let output = directory.path().join("routed.protocraft");
    let mut project = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    let board = project.board.as_mut().unwrap();
    board.tracks.clear();
    board.vias.clear();
    eda_storage::create_new(&input, &project).unwrap();
    let result = cli(&[
        "pcb-route",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--max-expansions",
        "100000",
        "--passes",
        "2",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["stats"]["search_expansions"].as_u64().unwrap() <= 100_000);
    assert!(
        report["stats"]["routing_passes_attempted"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let invalid = directory.path().join("invalid.protocraft");
    assert!(
        !cli(&[
            "pcb-route",
            input.to_str().unwrap(),
            "--output",
            invalid.to_str().unwrap(),
            "--passes",
            "0"
        ])
        .status
        .success()
    );
    assert!(!invalid.exists());
}
