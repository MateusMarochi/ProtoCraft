use eda_core::{Board, MountingHole, NmPoint, Project};
use eda_pcb::dfm::{ManufacturingLimits, check};

#[test]
fn manufacturing_limits_are_explicit_and_do_not_mutate_board_rules() {
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
    let report = check(&project, &ManufacturingLimits::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "DFM_NPTH_DRILL")
    );
    let permissive = ManufacturingLimits {
        min_npth_drill_nm: 300_000,
        ..Default::default()
    };
    assert!(
        !check(&project, &permissive)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "DFM_NPTH_DRILL")
    );
    assert_eq!(before, serde_json::to_value(&project).unwrap());
}

#[test]
fn manufacturing_limits_reject_negative_and_unbounded_values() {
    let project = Project::empty("DFM");
    for width in [-1, 0, 50_000_001] {
        let limits = ManufacturingLimits {
            min_track_width_nm: width,
            ..Default::default()
        };
        assert!(check(&project, &limits).is_err());
    }
}

#[test]
fn qfn_escape_vias_are_not_exempt_from_manufacturing_limits() {
    let project = eda_library::buck_pi4_project().unwrap();
    let limits = ManufacturingLimits::default();
    let report = check(&project, &limits).unwrap();
    assert!(report.diagnostics.iter().any(|d| d.code == "DFM_VIA_DRILL"));
    let relaxed = ManufacturingLimits {
        min_via_drill_nm: 200_000,
        ..limits.clone()
    };
    assert!(
        !check(&project, &relaxed)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "DFM_VIA_DRILL")
    );
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::to_value(check(&project, &limits).unwrap()).unwrap()
    );
}

#[test]
fn slots_use_tool_width_and_report_pad_ring_separately() {
    let mut project = eda_library::buck_pi4_project().unwrap();
    let pad = &mut project
        .components
        .get_mut(&eda_core::ComponentId(1))
        .unwrap()
        .footprint
        .as_mut()
        .unwrap()
        .pads[0];
    let number = pad.number.clone();
    pad.shape = eda_core::PadShape::Rect;
    pad.size = NmPoint::new(1_000_000, 1_600_000);
    pad.drill = Some(NmPoint::new(600_000, 1_200_000));
    let limits = ManufacturingLimits {
        min_pth_drill_nm: 700_000,
        min_annular_ring_nm: 210_000,
        ..Default::default()
    };
    let report = check(&project, &limits).unwrap();
    let diagnostics: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| {
            d.components == [eda_core::ComponentId(1)] && d.pad_number.as_ref() == Some(&number)
        })
        .collect();
    assert!(diagnostics.iter().any(|d| d.code == "DFM_PAD_DRILL"
        && d.actual_nm == Some(600_000)
        && d.minimum_nm == Some(700_000)));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "DFM_ANNULAR_RING" && d.actual_nm == Some(200_000))
    );
    let boundary = ManufacturingLimits {
        min_pth_drill_nm: 600_000,
        min_annular_ring_nm: 200_000,
        ..limits
    };
    let result = check(&project, &boundary).unwrap();
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.components == [eda_core::ComponentId(1)]
                && d.pad_number.as_ref() == Some(&number)
                && d.code.starts_with("DFM_"))
    );
}

#[test]
fn custom_plated_pad_requires_manual_review_and_marks_incomplete() {
    let mut project = eda_library::buck_pi4_project().unwrap();
    let pad = &mut project
        .components
        .get_mut(&eda_core::ComponentId(1))
        .unwrap()
        .footprint
        .as_mut()
        .unwrap()
        .pads[0];
    pad.shape = eda_core::PadShape::Custom;
    pad.polygons = vec![vec![
        NmPoint::new(-500_000, -500_000),
        NmPoint::new(500_000, -500_000),
        NmPoint::new(0, 500_000),
    ]];
    let report = check(&project, &ManufacturingLimits::default()).unwrap();
    assert!(!report.complete);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "DFM_CUSTOM_PAD_REVIEW"
                && d.severity == "warning"
                && d.pad_number.is_some())
    );
}

#[test]
fn absent_board_is_rejected_and_holes_are_identifiable_at_boundary() {
    assert!(check(&Project::empty("DFM"), &ManufacturingLimits::default()).is_err());
    let mut project = Project::empty("DFM");
    let mut board = Board::default();
    board.holes.insert(
        7,
        MountingHole {
            id: 7,
            position: NmPoint::new(10_000_000, 10_000_000),
            drill_nm: 499_999,
        },
    );
    project.board = Some(Box::new(board));
    let report = check(&project, &ManufacturingLimits::default()).unwrap();
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "DFM_NPTH_DRILL")
        .unwrap();
    assert_eq!(d.object_kind.as_deref(), Some("hole"));
    assert_eq!(d.object_id, Some(7));
    assert_eq!((d.actual_nm, d.minimum_nm), (Some(499_999), Some(500_000)));
    project
        .board
        .as_mut()
        .unwrap()
        .holes
        .get_mut(&7)
        .unwrap()
        .drill_nm = 500_000;
    assert!(
        !check(&project, &ManufacturingLimits::default())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "DFM_NPTH_DRILL")
    );
}

#[test]
fn generic_limits_never_relax_existing_net_classes() {
    let mut project = eda_library::buck_pi4_project().unwrap();
    project
        .board
        .as_mut()
        .unwrap()
        .net_classes
        .get_mut("OUTPUT_3A")
        .unwrap()
        .track_width_nm = 2_000_000;
    let limits = ManufacturingLimits {
        min_track_width_nm: 100_000,
        min_clearance_nm: 0,
        ..Default::default()
    };
    let report = check(&project, &limits).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "NETCLASS_TRACK_WIDTH")
    );
}
