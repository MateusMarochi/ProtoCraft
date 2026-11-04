use eda_core::{Command, Engine, NmPoint};
use eda_pcb::cleanup::{CleanupOptions, plan_with_options};

#[test]
fn split_track_cleanup_preserves_connectivity_and_undo() {
    let mut project = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    let board = project.board.as_mut().unwrap();
    let id = *board.tracks.keys().next().unwrap();
    let mut track = board.tracks[&id].clone();
    let end = track.end;
    let midpoint = NmPoint::new((track.start.x + end.x) / 2, (track.start.y + end.y) / 2);
    track.end = midpoint;
    board.tracks.insert(id, track.clone());
    let next = board.tracks.keys().max().unwrap() + 1;
    track.id = next;
    track.start = midpoint;
    track.end = end;
    board.tracks.insert(next, track);
    let original = project.clone();
    let before = eda_pcb::check(&project).unwrap();
    let plan = plan_with_options(
        &project,
        &CleanupOptions {
            merge_collinear: true,
        },
    )
    .unwrap();
    assert!(plan.merged_tracks >= 1);
    let mut engine = Engine::new(project).unwrap();
    engine.execute(Command::Batch(plan.commands)).unwrap();
    let after = eda_pcb::check(engine.project()).unwrap();
    assert_eq!(before.airwires.len(), after.airwires.len());
    let cleaned = engine.project().clone();
    assert!(engine.undo());
    assert_eq!(engine.project(), &original);
    assert!(engine.redo());
    assert_eq!(engine.project(), &cleaned);
}
use eda_core::{BoardSide, Project, Via};
fn pair() -> Project {
    let mut p = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    let b = p.board.as_mut().unwrap();
    let mut t = b.tracks.values().next().unwrap().clone();
    b.tracks.clear();
    b.vias.clear();
    b.holes.clear();
    t.id = 1;
    t.start = NmPoint::new(15_000_000, 25_000_000);
    t.end = NmPoint::new(20_000_000, 25_000_000);
    t.width_nm = 250_000;
    t.layer = BoardSide::Top;
    b.tracks.insert(1, t.clone());
    t.id = 2;
    t.start = t.end;
    t.end = NmPoint::new(25_000_000, 25_000_000);
    b.tracks.insert(2, t);
    p
}
fn count(p: &Project) -> usize {
    plan_with_options(
        p,
        &CleanupOptions {
            merge_collinear: true,
        },
    )
    .unwrap()
    .merged_tracks
}
#[test]
fn protects_branch_via_and_pad_junctions() {
    let p = pair();
    assert_eq!(count(&p), 1);
    let mut branch = p.clone();
    let b = branch.board.as_mut().unwrap();
    let mut t = b.tracks[&2].clone();
    t.id = 3;
    t.end = NmPoint::new(20_000_000, 27_000_000);
    b.tracks.insert(3, t);
    assert_eq!(count(&branch), 0);
    let mut via = p.clone();
    let b = via.board.as_mut().unwrap();
    b.vias.insert(
        1,
        Via {
            id: 1,
            position: b.tracks[&1].end,
            diameter_nm: 600_000,
            drill_nm: 300_000,
            net: b.tracks[&1].net,
        },
    );
    assert_eq!(count(&via), 0);
    let mut pad = p.clone();
    let center = eda_pcb::placed_pads(&pad).unwrap()[0].position;
    let b = pad.board.as_mut().unwrap();
    b.tracks.get_mut(&1).unwrap().start = NmPoint::new(center.x - 1_000_000, center.y);
    b.tracks.get_mut(&1).unwrap().end = center;
    b.tracks.get_mut(&2).unwrap().start = center;
    b.tracks.get_mut(&2).unwrap().end = NmPoint::new(center.x + 1_000_000, center.y);
    assert_eq!(count(&pad), 0);
}
#[test]
fn rejects_different_layer_width_net_and_disjoint_segments() {
    let p = pair();
    let mut different = p.clone();
    different
        .board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&2)
        .unwrap()
        .layer = BoardSide::Bottom;
    assert_eq!(count(&different), 0);
    let mut different = p.clone();
    different
        .board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&2)
        .unwrap()
        .width_nm = 300_000;
    assert_eq!(count(&different), 0);
    let mut different = p.clone();
    different
        .board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&2)
        .unwrap()
        .net
        .pin = 1;
    assert_eq!(count(&different), 0);
    let mut different = p;
    different
        .board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&2)
        .unwrap()
        .start
        .x += 1_000_000;
    assert_eq!(count(&different), 0);
}
#[test]
fn removes_overlap_and_contained_segment_without_changing_coverage() {
    let mut p = pair();
    p.board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&2)
        .unwrap()
        .start
        .x = 18_000_000;
    assert_eq!(count(&p), 1);
    let b = p.board.as_mut().unwrap();
    b.tracks.get_mut(&2).unwrap().end.x = 19_000_000;
    assert_eq!(count(&p), 1);
}
#[test]
fn rejects_excessive_merge_work_without_mutation() {
    let mut p = pair();
    let b = p.board.as_mut().unwrap();
    let t = b.tracks[&1].clone();
    for id in 3..=257 {
        let mut t = t.clone();
        t.id = id;
        b.tracks.insert(id, t);
    }
    let before = p.clone();
    assert!(
        plan_with_options(
            &p,
            &CleanupOptions {
                merge_collinear: true
            }
        )
        .is_err()
    );
    assert_eq!(p, before);
}
#[test]
fn does_not_merge_two_valid_pad_escapes_into_an_invalid_long_track() {
    use eda_core::{NetClass, PadEscapeRule, PinRef};
    use std::collections::BTreeSet;
    let mut p = pair();
    let ids: Vec<_> = p.components.keys().copied().collect();
    let first = ids[0];
    let second = ids[1];
    let mut engine = Engine::new(p).unwrap();
    for id in [first, second] {
        let pos = engine.project().components[&id].pins()[0].position;
        engine
            .execute(Command::AddLabel {
                position: pos,
                name: "ESCAPE".into(),
            })
            .unwrap();
    }
    p = engine.project().clone();
    let b = p.board.as_mut().unwrap();
    b.tracks.clear();
    let placement = b.placements[&first];
    let mut target = placement;
    target.position.y += 3_000_000;
    b.placements.insert(second, target);
    let pads = eda_pcb::placed_pads(&p).unwrap();
    let a = pads
        .iter()
        .find(|v| v.component == first && v.number == "1")
        .unwrap()
        .position;
    let z = pads
        .iter()
        .find(|v| v.component == second && v.number == "1")
        .unwrap()
        .position;
    assert_eq!(z.y - a.y, 3_000_000);
    let middle = NmPoint::new(a.x, a.y + 1_500_000);
    let pin = PinRef {
        component: first,
        pin: 0,
    };
    let nl = eda_netlist::build_netlist(&p);
    let net = nl.net_for_pin(pin).unwrap();
    let name = nl.nets.iter().find(|n| n.id == net).unwrap().name.clone();
    let b = p.board.as_mut().unwrap();
    b.net_classes.clear();
    b.net_classes.insert(
        "POWER".into(),
        NetClass {
            net_names: BTreeSet::from([name]),
            track_width_nm: 800_000,
            clearance_nm: 100_000,
            via_diameter_nm: 800_000,
            via_drill_nm: 400_000,
            pad_escape: Some(PadEscapeRule {
                track_width_nm: 250_000,
                max_length_nm: 2_000_000,
                via_diameter_nm: 400_000,
                via_drill_nm: 200_000,
            }),
        },
    );
    for (id, start, end) in [(1, a, middle), (2, middle, z)] {
        b.tracks.insert(
            id,
            eda_core::Track {
                id,
                start,
                end,
                width_nm: 250_000,
                layer: BoardSide::Top,
                net: pin,
            },
        );
    }
    let before = eda_pcb::check(&p).unwrap();
    assert!(
        !before
            .diagnostics
            .iter()
            .any(|d| d.code == "NETCLASS_TRACK_WIDTH")
    );
    assert_eq!(count(&p), 0);
}
#[test]
fn protects_offcenter_terminal_inside_rotated_pad_copper() {
    let mut p = pair();
    let id = *p.components.keys().next().unwrap();
    p.components
        .get_mut(&id)
        .unwrap()
        .footprint
        .as_mut()
        .unwrap()
        .pads[0]
        .rotation_mdeg = 45_000;
    let pad = eda_pcb::placed_pads(&p)
        .unwrap()
        .into_iter()
        .find(|pad| pad.component == id && pad.number == "1")
        .unwrap();
    let junction = NmPoint::new(pad.position.x + 100_000, pad.position.y + 100_000);
    let b = p.board.as_mut().unwrap();
    let a = b.tracks.get_mut(&1).unwrap();
    a.start = NmPoint::new(junction.x - 2_000_000, junction.y);
    a.end = junction;
    let z = b.tracks.get_mut(&2).unwrap();
    z.start = junction;
    z.end = NmPoint::new(junction.x + 2_000_000, junction.y);
    assert_ne!(junction.y, pad.position.y);
    assert_eq!(count(&p), 0);
}
