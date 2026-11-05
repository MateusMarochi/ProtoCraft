use super::*;
struct Drill {
    position: NmPoint,
    axis: [NmPoint; 2],
    radius: f64,
    net: Option<NetId>,
    owner: String,
}
fn collect_drills(project: &Project, pads: &[PlacedPad]) -> Result<Vec<Drill>, String> {
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let netlist = build_netlist(project);
    let mut drills = Vec::new();
    let mut seen = BTreeSet::new();
    for p in pads {
        if let Some(size) = p.pad.drill {
            let owner = format!(
                "{}:{}",
                project.components[&p.component].reference, p.number
            );
            if seen.insert((owner.clone(), p.position.x, p.position.y)) {
                let placement = &board.placements[&p.component];
                let half = (size.x - size.y).unsigned_abs() as f64 / 2.0;
                let angle = (p.pad.rotation_mdeg as f64 / 1000.0).to_radians();
                let (sin, cos) = angle.sin_cos();
                let (dx, dy) = if size.x >= size.y {
                    (half * cos, half * sin)
                } else {
                    (-half * sin, half * cos)
                };
                let endpoint = |sign: f64| {
                    placement.transform(NmPoint::new(
                        p.pad.position.x + (sign * dx).round() as i64,
                        p.pad.position.y + (sign * dy).round() as i64,
                    ))
                };
                drills.push(Drill {
                    position: p.position,
                    axis: [endpoint(-1.0), endpoint(1.0)],
                    radius: size.x.min(size.y) as f64 / 2.0,
                    net: if p.pad.technology == PadTechnology::NonPlated {
                        None
                    } else {
                        p.net
                    },
                    owner,
                });
            }
        }
    }
    for v in board.vias.values() {
        drills.push(Drill {
            position: v.position,
            axis: [v.position; 2],
            radius: v.drill_nm as f64 / 2.0,
            net: netlist.net_for_pin(v.net),
            owner: format!("via {}", v.id),
        });
    }
    for h in board.holes.values() {
        drills.push(Drill {
            position: h.position,
            axis: [h.position; 2],
            radius: h.drill_nm as f64 / 2.0,
            net: None,
            owner: format!("furo {}", h.id),
        });
    }
    Ok(drills)
}

pub(super) fn check_drill_clearance(
    project: &Project,
    pads: &[PlacedPad],
    clearance_nm: i64,
) -> Result<Vec<PcbDiagnostic>, String> {
    let drills = collect_drills(project, pads)?;
    let mut diagnostics = Vec::new();
    let mut work = 0usize;
    for (i, d) in drills.iter().enumerate() {
        for other in &drills[i + 1..] {
            work += 1;
            if work > 2_000_000 {
                return Err(eda_i18n::text(
                    "DRC de furação excedeu o orçamento de geometria.",
                ));
            }
            let gap = segment_distance(d.axis[0], d.axis[1], other.axis[0], other.axis[1])
                - d.radius
                - other.radius;
            if gap <= 0.0 || gap < clearance_nm as f64 - 1.0 {
                if diagnostics.len() >= 1000 {
                    return Err(eda_i18n::text("DRC_DRILL_DIAGNOSTIC_LIMIT"));
                }
                diagnostics.push(PcbDiagnostic {
                    severity: "error".into(),
                    code: "HOLE_CLEARANCE".into(),
                    message: eda_i18n::format(
                        "{owner} e {other} violam a folga entre furos.",
                        &[("owner", d.owner.clone()), ("other", other.owner.clone())],
                    ),
                    components: vec![],
                    position: Some(d.position),
                });
            }
        }
    }
    Ok(diagnostics)
}

pub(super) fn check_drills(
    project: &Project,
    pads: &[PlacedPad],
    atoms: &[Copper],
) -> Result<Vec<PcbDiagnostic>, String> {
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let drills = collect_drills(project, pads)?;
    let mut diagnostics = check_drill_clearance(project, pads, board.clearance_nm)?;
    let mut truncated = false;
    let mut add = |code: &str, message: String, position: NmPoint| {
        if diagnostics.len() < 1000 {
            diagnostics.push(PcbDiagnostic {
                severity: "error".into(),
                code: code.into(),
                message,
                components: vec![],
                position: Some(position),
            });
        } else {
            truncated = true;
        }
    };
    let mut work = 0usize;
    for d in &drills {
        let edge = d.radius + board.edge_clearance_nm as f64;
        if d.axis
            .iter()
            .any(|p| !board.contains_with_clearance(*p, edge.ceil() as i64))
        {
            add(
                "HOLE_EDGE",
                format!(
                    "{owner} viola a distância de furação à borda.",
                    owner = d.owner
                ),
                d.position,
            );
        }
        let circle = Copper {
            points: d.axis.to_vec(),
            radius: d.radius,
            filled: false,
            layers: 3,
            net: None,
            component: None,
            label: String::new(),
        };
        for atom in atoms {
            if atom.label == d.owner || (d.net.is_some() && d.net == atom.net) {
                continue;
            }
            work = work.saturating_add(atom.points.len().max(1));
            if work > 2_000_000 {
                return Err("DRC de furação excedeu o orçamento de geometria.".into());
            }
            let gap = distance(&circle, atom);
            if gap <= 0.0 || gap < board.clearance_nm as f64 - 1.0 {
                add(
                    "HOLE_COPPER",
                    format!(
                        "{owner} invade a folga de cobre de {item}.",
                        owner = d.owner,
                        item = atom.label
                    ),
                    d.position,
                );
            }
        }
    }
    for h in board.holes.values() {
        let circle = Copper {
            points: vec![h.position],
            radius: h.drill_nm as f64 / 2.0,
            filled: false,
            layers: 3,
            net: None,
            component: None,
            label: String::new(),
        };
        for (id, placement) in &board.placements {
            for k in &project.components[id]
                .footprint
                .as_ref()
                .ok_or("Footprint ausente")?
                .keepouts
            {
                work = work.saturating_add(k.polygon.len());
                if work > 2_000_000 {
                    return Err("DRC de furação excedeu o orçamento de geometria.".into());
                }
                let zone = Copper {
                    points: k.polygon.iter().map(|p| placement.transform(*p)).collect(),
                    radius: 0.0,
                    filled: true,
                    layers: 3,
                    net: None,
                    component: Some(*id),
                    label: String::new(),
                };
                if distance(&circle, &zone) <= 0.0 {
                    add(
                        "HOLE_KEEPOUT",
                        format!(
                            "Furo {id} invade área de exclusão de {reference}.",
                            id = h.id,
                            reference = project.components[id].reference
                        ),
                        h.position,
                    );
                }
            }
        }
    }
    if truncated {
        return Err(eda_i18n::text("DRC_DRILL_DIAGNOSTIC_LIMIT"));
    }
    Ok(diagnostics)
}

#[cfg(test)]
mod capsule_tests {
    use super::*;

    #[test]
    fn diagnostic_cap_returns_error_instead_of_partial_success() {
        let mut project = Project::empty("drill cap");
        let mut board = Board::default();
        for id in 1..=1001 {
            board.holes.insert(
                id,
                MountingHole {
                    id,
                    position: NmPoint::new(board.width_nm + id as i64 * 2_000_000, 10_000_000),
                    drill_nm: 500_000,
                },
            );
        }
        project.board = Some(Box::new(board));
        assert!(check_drills(&project, &[], &[]).is_err());
    }

    fn slot(rotation: u8, side: BoardSide, pad_angle: i32) -> Project {
        let mut project = eda_library::buck_pi4_project().unwrap();
        for component in project.components.values_mut() {
            if let Some(footprint) = component.footprint.as_mut() {
                for pad in &mut footprint.pads {
                    pad.drill = None;
                    pad.technology = PadTechnology::Smd;
                    pad.layers = vec!["F.Cu".into()];
                }
            }
        }
        let id = *project
            .board
            .as_ref()
            .unwrap()
            .placements
            .keys()
            .next()
            .unwrap();
        let pad = &mut project
            .components
            .get_mut(&id)
            .unwrap()
            .footprint
            .as_mut()
            .unwrap()
            .pads[0];
        pad.drill = Some(NmPoint::new(4_000_000, 1_000_000));
        pad.size = NmPoint::new(5_000_000, 2_000_000);
        pad.technology = PadTechnology::ThroughHole;
        pad.layers = vec!["*.Cu".into()];
        pad.rotation_mdeg = pad_angle;
        let board = project.board.as_mut().unwrap();
        board.vias.clear();
        board.holes.clear();
        board.placements.get_mut(&id).unwrap().rotation = rotation;
        board.placements.get_mut(&id).unwrap().side = side;
        project
    }

    #[test]
    fn slot_axis_obeys_pad_rotation_placement_and_side() {
        for side in [BoardSide::Top, BoardSide::Bottom] {
            for (rotation, angle, vertical) in [
                (0, 0, false),
                (1, 0, true),
                (0, 90_000, true),
                (1, 90_000, false),
            ] {
                let project = slot(rotation, side, angle);
                let pads = placed_pads(&project).unwrap();
                let drills = collect_drills(&project, &pads).unwrap();
                assert_eq!(drills.len(), 1);
                let d = &drills[0];
                assert_eq!(d.radius, 500_000.0);
                assert_eq!(d.axis[0].x == d.axis[1].x, vertical);
                assert!((segment_point(d.position, d.axis[0], d.axis[1])).abs() < 0.1);
            }
        }
    }

    #[test]
    fn slot_clearance_uses_capsule_not_enclosing_circle() {
        let mut project = slot(0, BoardSide::Top, 0);
        let pads = placed_pads(&project).unwrap();
        let center = collect_drills(&project, &pads).unwrap()[0].position;
        project.board.as_mut().unwrap().holes.insert(
            991,
            MountingHole {
                id: 991,
                position: NmPoint::new(center.x, center.y + 1_250_000),
                drill_nm: 1_000_000,
            },
        );
        assert!(
            check_drill_clearance(&project, &pads, 250_000)
                .unwrap()
                .is_empty()
        );
        project
            .board
            .as_mut()
            .unwrap()
            .holes
            .get_mut(&991)
            .unwrap()
            .position
            .y -= 2;
        assert_eq!(
            check_drill_clearance(&project, &pads, 250_000)
                .unwrap()
                .len(),
            1
        );
    }
}
