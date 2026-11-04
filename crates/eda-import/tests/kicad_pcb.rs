use eda_core::{BoardSide, NM_PER_MM, PadShape, PadTechnology};
use eda_import::kicad::{import_kicad_pcb, parse_kicad_pcb};
use eda_netlist::build_netlist;

fn board() -> String {
    r#"(kicad_pcb
      (version 20240108)
      (generator "pcbnew")
      (general (thickness 1.6))
      (net 0 "")
      (net 1 "GND")
      (net 2 "VCC")
      (footprint "Device:R_Small" (layer "F.Cu") (at 20 20 0)
        (property "Reference" "R1")
        (property "Value" "10k")
        (fp_line (start -1 -0.5) (end 1 -0.5)
          (stroke (width 0.05) (type default)) (layer "F.CrtYd"))
        (fp_line (start 1 -0.5) (end 1 0.5)
          (stroke (width 0.05) (type default)) (layer "F.CrtYd"))
        (fp_line (start 1 0.5) (end -1 0.5)
          (stroke (width 0.05) (type default)) (layer "F.CrtYd"))
        (fp_line (start -1 0.5) (end -1 -0.5)
          (stroke (width 0.05) (type default)) (layer "F.CrtYd"))
        (pad "1" smd roundrect (at -0.8 0 0) (size 0.8 0.9)
          (layers "F.Cu" "F.Mask" "F.Paste") (roundrect_rratio 0.25)
          (solder_mask_margin 0.01) (net 1 "GND"))
        (pad "2" smd rect (at 0.8 0 0) (size 0.8 0.9)
          (layers "F.Cu" "F.Mask" "F.Paste") (net 2 "VCC")))
      (segment (start 20 20) (end 25 20) (width 0.25) (layer "F.Cu") (net 1))
      (via (at 25 20) (size 0.6) (drill 0.3)
        (layers "F.Cu" "B.Cu") (net 1))
      (gr_rect (start 0 0) (end 50 40)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#
        .to_owned()
}

#[test]
fn importa_footprint_pads_rede_trilha_via_e_contorno_retangular() {
    let result = parse_kicad_pcb("resistor", &board()).expect("fixture deve ser importável");
    assert_eq!(result.project.components.len(), 1);
    assert!(!result.report.electrical_complete);
    assert!(result.report.geometry_complete);

    let component = result.project.components.values().next().unwrap();
    assert_eq!(component.reference, "R1");
    assert_eq!(component.imported.as_ref().unwrap().value_text, "10k");
    let footprint = component.footprint.as_ref().unwrap();
    assert_eq!(footprint.pads.len(), 2);
    assert_eq!(footprint.pads[0].shape, PadShape::RoundRect);
    assert_eq!(footprint.pads[0].technology, PadTechnology::Smd);
    assert_eq!(footprint.pads[0].mask_margin_nm, 10_000);

    let board = result.project.board.as_ref().unwrap();
    assert_eq!(board.width_nm, 50 * NM_PER_MM);
    assert_eq!(board.height_nm, 40 * NM_PER_MM);
    assert_eq!(board.placements[&component.id].position.x, 20 * NM_PER_MM);
    assert_eq!(board.tracks.len(), 1);
    assert_eq!(board.vias.len(), 1);
    assert_eq!(board.tracks.values().next().unwrap().layer, BoardSide::Top);
    let nets = build_netlist(&result.project);
    assert_eq!(
        nets.nets
            .iter()
            .find(|net| net.name == "GND")
            .unwrap()
            .pins
            .len(),
        1
    );
    assert_eq!(
        nets.nets
            .iter()
            .find(|net| net.name == "VCC")
            .unwrap()
            .pins
            .len(),
        1
    );
}

#[test]
fn registra_perda_para_zonas_e_contorno_nao_retangular() {
    let mut input = board();
    input = input.replace(
        "(gr_rect (start 0 0) (end 50 40)\n        (stroke (width 0.05) (type default)) (fill none) (layer \"Edge.Cuts\"))",
        "(gr_line (start 0 0) (end 50 0) (stroke (width 0.05) (type default)) (layer \"Edge.Cuts\"))\n      (gr_line (start 50 0) (end 40 40) (stroke (width 0.05) (type default)) (layer \"Edge.Cuts\"))\n      (gr_line (start 40 40) (end 0 40) (stroke (width 0.05) (type default)) (layer \"Edge.Cuts\"))\n      (gr_line (start 0 40) (end 0 0) (stroke (width 0.05) (type default)) (layer \"Edge.Cuts\"))",
    );
    input = input.replace(
        "(net 2 \"VCC\")",
        "(net 2 \"VCC\") (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))",
    );
    let result = parse_kicad_pcb("lossy", &input).expect("subconjunto deve ser importável");
    assert!(!result.report.lossless());
    assert!(!result.report.geometry_complete);
    assert!(
        result
            .report
            .losses
            .iter()
            .any(|issue| issue.code == "outline_approximated")
    );
    assert!(
        result
            .report
            .losses
            .iter()
            .any(|issue| issue.code == "copper_zone_omitted")
    );
}

#[test]
fn rejeita_s_expression_invalida_cabecalho_errado_e_arquivo_grande() {
    assert!(parse_kicad_pcb("x", "(kicad_pcb (version 20240108)").is_err());
    assert!(parse_kicad_pcb("x", "(kicad_sch (version 20250114))").is_err());
    assert!(parse_kicad_pcb("x", &" ".repeat(8 * 1024 * 1024 + 1)).is_err());

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("oversized.kicad_pcb");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert!(import_kicad_pcb(path).is_err());
}

#[test]
fn importador_de_arquivo_le_o_conteudo_sem_modificar_a_origem() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("board.kicad_pcb");
    let input = board();
    std::fs::write(&path, &input).unwrap();
    let result = import_kicad_pcb(&path).unwrap();
    assert_eq!(result.project.name, "board");
    assert_eq!(std::fs::read_to_string(path).unwrap(), input);
}

#[test]
fn converte_footprint_np_thru_hole_de_montagem_em_furo_da_placa() {
    let input = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
      (footprint "MountingHole:MountingHole_3.2mm" (layer "F.Cu") (at 12 8 0)
        (property "Reference" "H1") (property "Value" "MountingHole")
        (pad "" np_thru_hole circle (at 0 0) (size 3.2 3.2) (drill 3.2)
          (layers "*.Cu" "*.Mask")))
      (gr_rect (start 0 0) (end 30 20)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#;
    let result = parse_kicad_pcb("hole", input).unwrap();
    let board = result.project.board.as_ref().unwrap();
    assert_eq!(board.holes.len(), 1);
    let hole = board.holes.values().next().unwrap();
    assert_eq!(hole.position.x, 12 * NM_PER_MM);
    assert_eq!(hole.position.y, 8 * NM_PER_MM);
    assert_eq!(hole.drill_nm, 3_200_000);
    assert!(result.project.components.is_empty());
}

#[test]
fn pads_sem_rede_nao_sao_conectados_entre_si() {
    let input = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
      (net 0 "")
      (footprint "Device:R_Small" (layer "F.Cu") (at 10 10 0)
        (property "Reference" "R1") (property "Value" "10k")
        (pad "1" smd rect (at -1 0 0) (size 0.8 0.8) (layers "F.Cu" "F.Mask"))
        (pad "2" smd rect (at 1 0 0) (size 0.8 0.8) (layers "F.Cu" "F.Mask")))
      (gr_rect (start 0 0) (end 30 20)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#;
    let result = parse_kicad_pcb("unconnected", input).unwrap();
    let nets = build_netlist(&result.project);
    assert!(!result.report.geometry_complete);
    assert!(
        result
            .report
            .losses
            .iter()
            .any(|issue| { issue.code == "courtyard_derived_bounds" })
    );
    assert_eq!(nets.nets.len(), 2);
    assert!(nets.nets.iter().all(|net| net.pins.len() == 1));
    assert!(nets.nets.iter().all(|net| net.name != "0"));
}

#[test]
fn footprint_bottom_rotacionado_preserva_posicao_fisica_dos_pads() {
    let input = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
      (net 0 "")
      (footprint "Test:Bottom" (layer "B.Cu") (at 20 20 90)
        (property "Reference" "U1") (property "Value" "Test")
        (fp_line (start -2 -2) (end 2 -2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start 2 -2) (end 2 2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start 2 2) (end -2 2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start -2 2) (end -2 -2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (pad "1" smd rect (at 2 0 0) (size 0.8 0.8) (layers "B.Cu" "B.Mask"))
        (pad "2" smd rect (at 3 1 0) (size 0.8 0.8) (layers "B.Cu" "B.Mask")))
      (gr_rect (start 0 0) (end 50 40)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#;
    let result = parse_kicad_pcb("bottom", input).unwrap();
    let component = result.project.components.values().next().unwrap();
    let board = result.project.board.as_ref().unwrap();
    let placement = board.placements[&component.id];
    let footprint = component.footprint.as_ref().unwrap();
    let first = placement.transform(footprint.pads[0].position);
    let second = placement.transform(footprint.pads[1].position);
    // DRC do KiCad confirma as coordenadas da placa; a SVG do verso é espelhada para visualização.
    assert_eq!(
        first,
        eda_core::NmPoint::new(20 * NM_PER_MM, 18 * NM_PER_MM)
    );
    assert_eq!(
        second,
        eda_core::NmPoint::new(21 * NM_PER_MM, 17 * NM_PER_MM)
    );
    assert_eq!(placement.side, BoardSide::Bottom);
    assert!(result.report.geometry_complete);
}

#[test]
fn aceita_angulos_kicad_com_seis_casas_decimais() {
    let input = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
      (net 0 "")
      (footprint "Test:Decimal" (layer "F.Cu") (at 10.000000 10.000000 0.000000)
        (property "Reference" "U1") (property "Value" "Decimal")
        (fp_line (start -2 -2) (end 2 -2) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
        (fp_line (start 2 -2) (end 2 2) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
        (fp_line (start 2 2) (end -2 2) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
        (fp_line (start -2 2) (end -2 -2) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
        (pad "1" thru_hole circle (at 0.000000 -1.250000 90.000000)
          (size 1.600000 1.600000) (layers "*.Cu" "F.Mask") (drill 0.800000)))
      (gr_rect (start 0 0) (end 30 20)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#;
    let result = parse_kicad_pcb("decimal", input).expect("ângulos decimais válidos");
    let component = result.project.components.values().next().unwrap();
    let pad = &component.footprint.as_ref().unwrap().pads[0];
    assert_eq!(pad.position, eda_core::NmPoint::new(0, -1_250_000));
    assert_eq!(pad.rotation_mdeg, 90_000);
}

#[test]
fn footprint_bottom_em_zero_graus_preserva_coordenadas_assimetricas() {
    let input = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
      (net 0 "")
      (footprint "Test:BottomZero" (layer "B.Cu") (at 20 20 0.000000)
        (property "Reference" "U1") (property "Value" "Bottom")
        (fp_line (start -4 -2) (end 4 -2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start 4 -2) (end 4 2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start 4 2) (end -4 2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (fp_line (start -4 2) (end -4 -2) (stroke (width 0.05) (type solid)) (layer "B.CrtYd"))
        (pad "1" thru_hole circle (at 2 0 0) (size 1.5 1.5) (layers "*.Cu" "*.Mask") (drill 0.8))
        (pad "2" thru_hole circle (at 3 1 0) (size 1.5 1.5) (layers "*.Cu" "*.Mask") (drill 0.8)))
      (gr_rect (start 0 0) (end 40 40)
        (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#;
    let result = parse_kicad_pcb("bottom-zero", input).expect("footprint inferior válido");
    let component = result.project.components.values().next().unwrap();
    let placement = result.project.board.as_ref().unwrap().placements[&component.id];
    let footprint = component.footprint.as_ref().unwrap();
    assert_eq!(
        placement.transform(footprint.pads[0].position),
        eda_core::NmPoint::new(22 * NM_PER_MM, 20 * NM_PER_MM)
    );
    assert_eq!(
        placement.transform(footprint.pads[1].position),
        eda_core::NmPoint::new(23 * NM_PER_MM, 21 * NM_PER_MM)
    );
}
