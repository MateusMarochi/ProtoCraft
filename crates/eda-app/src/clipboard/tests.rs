use super::*;
use eda_core::{Board, BoardSide, NmPoint};
fn app() -> ProtoCraft {
    let mut engine = Engine::new(Project::empty("Clipboard")).unwrap();
    engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(0, 0),
        })
        .unwrap();
    engine
        .execute(Command::SetComponent {
            id: ComponentId(1),
            reference: "Rcustom".into(),
            value: 3300.0,
        })
        .unwrap();
    engine
        .execute(Command::RotateComponent { id: ComponentId(1) })
        .unwrap();
    let mut app = ProtoCraft::new(engine, None, None, None, false, None);
    app.choose(Some(ObjectId::Component(ComponentId(1))));
    app
}
fn ctrl(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    }
}
fn frame(app: &mut ProtoCraft, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
            events,
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    )
}
fn copied(output: &egui::FullOutput) -> String {
    output
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.clone()),
            _ => None,
        })
        .expect("texto publicado no clipboard do sistema")
}
fn pcb_app() -> ProtoCraft {
    let mut app = app();
    let footprint = eda_library::catalog()
        .unwrap()
        .footprints
        .iter()
        .find(|(name, _)| name.contains("R_0805_"))
        .unwrap()
        .1
        .clone();
    app.engine
        .execute(Command::AssignFootprint {
            id: ComponentId(1),
            footprint: Box::new(footprint),
        })
        .unwrap();
    app.engine
        .execute(Command::SetBoard {
            board: Board::default(),
        })
        .unwrap();
    app.engine
        .execute(Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: Placement {
                position: NmPoint::new(20_000_000, 20_000_000),
                rotation: 1,
                side: BoardSide::Bottom,
            },
        })
        .unwrap();
    let pad = eda_pcb::placed_pads(app.engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.pin.is_some_and(|p| p.pin == 0))
        .unwrap();
    let end = NmPoint::new(
        pad.position.x,
        pad.position.y + (pad.position.y - 20_000_000).signum() * 4_000_000,
    );
    app.engine
        .execute(Command::AddTrack {
            track: Track {
                id: 1,
                start: pad.position,
                end,
                width_nm: 250000,
                layer: BoardSide::Bottom,
                net: pad.pin.unwrap(),
            },
        })
        .unwrap();
    app.engine
        .execute(Command::AddVia {
            via: Via {
                id: 1,
                position: end,
                diameter_nm: 600000,
                drill_nm: 300000,
                net: pad.pin.unwrap(),
            },
        })
        .unwrap();
    let report = eda_pcb::check(app.engine.project()).unwrap();
    assert_eq!(report.errors(), 0, "{:?}", report.diagnostics);
    app.pcb.active = true;
    app.pcb.selected = Some(ComponentId(1));
    app.selected = None;
    app
}
#[test]
fn native_copy_paste_preserves_snapshot_and_uses_system_clipboard() {
    let mut app = app();
    let ctx = egui::Context::default();
    let original = app.engine.project().components[&ComponentId(1)].clone();
    let output = frame(&mut app, &ctx, vec![egui::Event::Copy]);
    let text = copied(&output);
    assert!(text.starts_with(PREFIX));
    assert_eq!(decode(&text).unwrap().component, original);
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert_eq!(app.engine.project().components.len(), 2);
    let pasted = &app.engine.project().components[&ComponentId(2)];
    assert_eq!(pasted.value, 3300.0);
    assert_eq!(pasted.rotation, 1);
    assert_eq!(pasted.parameters, original.parameters);
    assert_eq!(pasted.position, Point::new(4, 4));
    assert_ne!(pasted.reference, original.reference);
    assert_eq!(app.selected, Some(ObjectId::Component(ComponentId(2))));
    assert!(app.engine.undo());
    assert_eq!(app.engine.project().components.len(), 1);
    assert!(app.engine.redo());
    assert_eq!(app.engine.project().components.len(), 2);
}
#[test]
fn native_cut_paste_moves_original_id_reference_and_undo_restores_each_step() {
    let mut app = app();
    let ctx = egui::Context::default();
    let before = app.engine.project().clone();
    let output = frame(&mut app, &ctx, vec![egui::Event::Cut]);
    let text = copied(&output);
    assert!(app.engine.project().components.is_empty());
    assert!(app.selected.is_none());
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    assert_eq!(app.engine.project().components.len(), 1);
    let moved = &app.engine.project().components[&ComponentId(1)];
    assert_eq!(moved.reference, "Rcustom");
    assert_eq!(moved.position, Point::new(4, 4));
    assert_eq!(moved.value, 3300.0);
    assert!(app.engine.undo());
    assert!(app.engine.project().components.is_empty());
    assert!(app.engine.undo());
    assert_eq!(app.engine.project(), &before);
    assert!(app.engine.redo());
    assert!(app.engine.redo());
    assert_eq!(app.engine.project().components.len(), 1);
}
#[test]
fn raw_shortcuts_and_native_events_do_not_duplicate_paste_or_cut() {
    let mut app = app();
    let ctx = egui::Context::default();
    let output = frame(&mut app, &ctx, vec![ctrl(Key::C), egui::Event::Copy]);
    let text = copied(&output);
    frame(
        &mut app,
        &ctx,
        vec![
            ctrl(Key::V),
            egui::Event::Paste(text.clone()),
            egui::Event::Paste(text),
        ],
    );
    assert_eq!(app.engine.project().components.len(), 2);
    frame(&mut app, &ctx, vec![ctrl(Key::X), egui::Event::Cut]);
    assert_eq!(app.engine.project().components.len(), 1);
    frame(&mut app, &ctx, vec![ctrl(Key::V)]);
    assert_eq!(app.engine.project().components.len(), 2);
    assert!(
        app.engine
            .project()
            .components
            .contains_key(&ComponentId(2))
    );
}
#[test]
fn pcb_copy_pastes_component_and_footprint_without_accidental_schematic_nets() {
    let mut app = pcb_app();
    let ctx = egui::Context::default();
    let source = app.engine.project().components[&ComponentId(1)].clone();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Copy]));
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    let project = app.engine.project();
    let board = project.board.as_ref().unwrap();
    assert_eq!(project.components.len(), 2);
    assert_eq!(board.placements.len(), 2);
    let pasted = &project.components[&ComponentId(2)];
    assert_eq!(pasted.footprint, source.footprint);
    assert_eq!(board.placements[&ComponentId(2)].side, BoardSide::Bottom);
    assert_eq!(board.placements[&ComponentId(2)].rotation, 1);
    let nets = build_netlist(project);
    for pin in [0, 1] {
        assert_ne!(
            nets.net_for_pin(eda_core::PinRef {
                component: ComponentId(1),
                pin
            }),
            nets.net_for_pin(eda_core::PinRef {
                component: ComponentId(2),
                pin
            })
        );
    }
    assert_eq!(app.pcb.selected, Some(ComponentId(2)));
    assert_eq!(board.vias.len(), 1);
    assert_eq!(board.tracks.len(), 1);
}
#[test]
fn pcb_cut_paste_preserves_logical_identity_and_restores_copper() {
    let mut app = pcb_app();
    let ctx = egui::Context::default();
    let before = app.engine.project().clone();
    let text = copied(&frame(&mut app, &ctx, vec![ctrl(Key::X)]));
    let board = app.engine.project().board.as_ref().unwrap();
    assert_eq!(app.engine.project().components.len(), 1);
    assert!(board.placements.is_empty());
    assert!(board.tracks.is_empty());
    assert!(board.vias.is_empty());
    frame(&mut app, &ctx, vec![ctrl(Key::V), egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    let board = app.engine.project().board.as_ref().unwrap();
    assert_eq!(app.engine.project().components.len(), 1);
    assert_eq!(board.tracks, before.board.as_ref().unwrap().tracks);
    assert_eq!(board.vias, before.board.as_ref().unwrap().vias);
    assert_ne!(
        board.placements[&ComponentId(1)].position,
        before.board.as_ref().unwrap().placements[&ComponentId(1)].position
    );
    assert_eq!(
        app.engine.project().components[&ComponentId(1)].reference,
        "Rcustom"
    );
    assert!(app.engine.undo());
    assert!(app.engine.undo());
    assert_eq!(app.engine.project(), &before);
}
#[test]
fn mcu_clipboard_transfers_pins_and_footprint_between_documents() {
    let mut source = ProtoCraft::new(
        Engine::new(eda_library::physical_example().unwrap()).unwrap(),
        None,
        None,
        None,
        false,
        None,
    );
    source.choose(Some(ObjectId::Component(ComponentId(1))));
    let ctx = egui::Context::default();
    let text = copied(&frame(&mut source, &ctx, vec![egui::Event::Copy]));
    let original = source.engine.project().components[&ComponentId(1)].clone();
    let mut target = app();
    frame(
        &mut target,
        &egui::Context::default(),
        vec![egui::Event::Paste(text)],
    );
    assert!(!target.status_error, "{}", target.status);
    let copied = &target.engine.project().components[&ComponentId(2)];
    assert_eq!(copied.device, original.device);
    assert_eq!(copied.footprint, original.footprint);
    assert_eq!(copied.imported, original.imported);
}
#[test]
fn invalid_or_plain_text_clipboard_never_pastes_stale_component() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![egui::Event::Copy]);
    let before = app.engine.project().clone();
    for text in [
        "texto qualquer".to_owned(),
        format!("{PREFIX}{{}}"),
        format!("{PREFIX}{}", " ".repeat(MAX_BYTES)),
    ] {
        frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
        assert!(app.status_error);
        assert_eq!(app.engine.project(), &before);
    }
}
#[test]
fn failed_restore_is_atomic_and_keeps_cut_available() {
    let mut app = app();
    let ctx = egui::Context::default();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Cut]));
    app.engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(20, 20),
        })
        .unwrap();
    app.engine
        .execute(Command::SetComponent {
            id: ComponentId(2),
            reference: "Rcustom".into(),
            value: 1000.0,
        })
        .unwrap();
    let before = app.engine.project().clone();
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(app.status_error);
    assert_eq!(app.engine.project(), &before);
    assert!(app.clipboard.cut.is_some());
}
#[test]
fn focused_text_editor_owns_copy_cut_paste() {
    let mut app = app();
    let ctx = egui::Context::default();
    let id = egui::Id::new("clipboard_text");
    let mut text = "texto normal".to_owned();
    let before = app.engine.project().clone();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).id(id))
                .request_focus();
        });
    });
    assert!(ctx.wants_keyboard_input());
    let mut text_frame = |events| {
        ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                app.shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut text).id(id));
                });
            },
        )
    };
    let output = text_frame(vec![ctrl(Key::A), egui::Event::Copy]);
    assert_eq!(copied(&output), "texto normal");
    let _ = text_frame(vec![egui::Event::Cut]);
    let _ = text_frame(vec![egui::Event::Paste("novo texto".into())]);
    assert_eq!(text, "novo texto");
    assert_eq!(app.engine.project(), &before);
    assert!(app.clipboard.payload.is_none());
}
#[test]
fn replacing_project_discards_cut_ownership_but_retains_copy() {
    let mut app = app();
    let ctx = egui::Context::default();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Cut]));
    app.replace(Project::empty("Outro"), None);
    assert!(app.clipboard.cut.is_none());
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    assert_eq!(app.engine.project().components.len(), 1);
}
#[test]
fn imported_component_metadata_and_pins_survive_clipboard() {
    let mut app = app();
    let ctx = egui::Context::default();
    let mut component = app.engine.project().components[&ComponentId(1)].clone();
    component.kind = ComponentKind::Imported;
    component.value = 0.0;
    component.imported = Some(eda_core::ImportedComponent {
        source_format: "Proteus".into(),
        device: "CUSTOM_DEVICE".into(),
        value_text: "Valor original".into(),
        properties: std::collections::BTreeMap::from([("MODEL".into(), "inert.bin".into())]),
        pins: vec![
            eda_core::ImportedPin {
                number: "1".into(),
                name: "IN".into(),
                offset: Point::new(-4, 0),
            },
            eda_core::ImportedPin {
                number: "2".into(),
                name: "OUT".into(),
                offset: Point::new(4, 0),
            },
        ],
    });
    app.engine
        .execute(Command::PasteComponent {
            component,
            position: Point::new(40, 40),
        })
        .unwrap();
    app.choose(Some(ObjectId::Component(ComponentId(2))));
    let original = app.engine.project().components[&ComponentId(2)].clone();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Copy]));
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    let pasted = &app.engine.project().components[&ComponentId(3)];
    assert_eq!(pasted.imported, original.imported);
    assert_eq!(pasted.rotation, original.rotation);
}
#[test]
fn pcb_cut_restore_remaps_copper_ids_created_while_component_was_cut() {
    let mut app = pcb_app();
    let ctx = egui::Context::default();
    let source = app.engine.project().components[&ComponentId(1)].clone();
    app.engine
        .execute(Command::PasteComponent {
            component: source,
            position: Point::new(40, 0),
        })
        .unwrap();
    app.engine
        .execute(Command::PlaceOnBoard {
            id: ComponentId(2),
            placement: Placement {
                position: NmPoint::new(70_000_000, 50_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        })
        .unwrap();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Cut]));
    let pad = eda_pcb::placed_pads(app.engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.component == ComponentId(2) && p.pin.is_some_and(|p| p.pin == 0))
        .unwrap();
    let end = NmPoint::new(pad.position.x - 4_000_000, pad.position.y);
    app.engine
        .execute(Command::AddTrack {
            track: Track {
                id: 1,
                start: pad.position,
                end,
                width_nm: 250000,
                layer: BoardSide::Top,
                net: pad.pin.unwrap(),
            },
        })
        .unwrap();
    app.engine
        .execute(Command::AddVia {
            via: Via {
                id: 1,
                position: end,
                diameter_nm: 600000,
                drill_nm: 300000,
                net: pad.pin.unwrap(),
            },
        })
        .unwrap();
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    let board = app.engine.project().board.as_ref().unwrap();
    assert_eq!(board.tracks.len(), 2);
    assert_eq!(board.vias.len(), 2);
    assert_eq!(board.tracks[&1].net.component, ComponentId(2));
    assert_eq!(board.tracks[&2].net.component, ComponentId(1));
    assert_eq!(board.vias[&1].net.component, ComponentId(2));
    assert_eq!(board.vias[&2].net.component, ComponentId(1));
}
#[test]
fn paste_uses_cursor_in_each_canvas_instead_of_fallback() {
    let mut schematic = app();
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
    schematic.canvas_rect = Some(rect);
    schematic.pan = Vec2::ZERO;
    schematic.zoom = 18.0;
    let pointer = rect.center() + Vec2::new(72.0, 54.0);
    let expected = schematic.placement_point(pointer, rect);
    let text = copied(&frame(&mut schematic, &ctx, vec![egui::Event::Copy]));
    frame(
        &mut schematic,
        &ctx,
        vec![egui::Event::PointerMoved(pointer), egui::Event::Paste(text)],
    );
    assert_eq!(
        schematic.engine.project().components[&ComponentId(2)].position,
        expected
    );
    let mut pcb = pcb_app();
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ctx| pcb.pcb_canvas(ctx),
    );
    let text = copied(&frame(&mut pcb, &ctx, vec![egui::Event::Copy]));
    frame(
        &mut pcb,
        &ctx,
        vec![
            egui::Event::PointerMoved(rect.center()),
            egui::Event::Paste(text),
        ],
    );
    assert!(!pcb.status_error, "{}", pcb.status);
    assert_eq!(
        pcb.engine.project().board.as_ref().unwrap().placements[&ComponentId(2)].position,
        NmPoint::new(50_000_000, 40_000_000)
    );
}
#[test]
fn schematic_cut_restores_physical_snapshot_without_moving_board_copper() {
    let mut app = pcb_app();
    app.pcb.active = false;
    app.choose(Some(ObjectId::Component(ComponentId(1))));
    let ctx = egui::Context::default();
    let board = app.engine.project().board.clone();
    let text = copied(&frame(&mut app, &ctx, vec![egui::Event::Cut]));
    assert!(app.engine.project().components.is_empty());
    frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
    assert!(!app.status_error, "{}", app.status);
    assert_eq!(app.engine.project().board, board);
    assert_eq!(
        app.engine.project().components[&ComponentId(1)].position,
        Point::new(4, 4)
    );
}
