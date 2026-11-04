use super::*;
use eda_core::{
    Board, BoardSide, Command, Component, ComponentId, ComponentKind, DataSource, DeviceDefinition,
    Engine, Footprint, FootprintLine, LabelId, MountingHole, NM_PER_MM, NetClass, NmBounds,
    NmPoint, Pad, PadShape, PadTechnology, Placement, Point, Project,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
pub fn handles(cmd: &str) -> bool {
    matches!(
        cmd,
        "devices"
            | "device"
            | "device-project"
            | "raspberry-pi-4-shield"
            | "raspberry-pi-hat"
            | "buck-pi4-12v"
            | "power-design"
            | "pcb-info"
            | "pcb-check"
            | "pcb-netclass"
            | "pcb-route"
            | "pcb-export"
            | "pcb-manufacture"
            | "pcb-assembly"
    )
}
fn json(value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn raspberry_pi_4_shield_project() -> Result<Project, Box<dyn Error>> {
    let (device, header_fp) = eda_library::raspberry_pi_4_gpio_definition();
    let source = DataSource {
        project: "ProtoCraft Raspberry Pi shield".into(),
        revision: "1".into(),
        url: "https://www.raspberrypi.com/documentation/computers/raspberry-pi.html".into(),
        license: "Original ProtoCraft prototype geometry".into(),
    };
    let resistor_fp = two_pad_footprint(
        "ProtoCraft_THT:R_Axial_7.62mm",
        "Resistor axial 7,62 mm, dois pads PTH",
        7_620_000,
        900_000,
        source.clone(),
    );
    let led_fp = two_pad_footprint(
        "ProtoCraft_THT:LED_3mm",
        "LED radial 3 mm, ânodo no pad 1",
        2_540_000,
        800_000,
        source.clone(),
    );
    let tp_fp = Footprint {
        id: "ProtoCraft_THT:TestPoint_1.8mm".into(),
        description: "Ponto de teste PTH GPIO17".into(),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-1_000_000, -1_000_000),
            max: NmPoint::new(1_000_000, 1_000_000),
        },
        pads: vec![Pad {
            number: "1".into(),
            position: NmPoint::default(),
            size: NmPoint::new(2_000_000, 2_000_000),
            rotation_mdeg: 0,
            shape: PadShape::Circle,
            technology: PadTechnology::ThroughHole,
            drill: Some(NmPoint::new(1_000_000, 1_000_000)),
            layers: vec!["*.Cu".into(), "F.Mask".into()],
            roundrect_ratio_ppm: 0,
            mask_margin_nm: 0,
            paste_margin_nm: 0,
            paste_ratio_ppm: 0,
            polygons: vec![],
        }],
        lines: vec![
            FootprintLine {
                start: NmPoint::new(-1_000_000, -1_000_000),
                end: NmPoint::new(1_000_000, 1_000_000),
                width_nm: 150_000,
                layer: "F.SilkS".into(),
            },
            FootprintLine {
                start: NmPoint::new(-1_000_000, 1_000_000),
                end: NmPoint::new(1_000_000, -1_000_000),
                width_nm: 150_000,
                layer: "F.SilkS".into(),
            },
        ],
        keepouts: vec![],
        source: source.clone(),
    };
    let ids = [
        ComponentId(1),
        ComponentId(2),
        ComponentId(3),
        ComponentId(4),
        ComponentId(5),
        ComponentId(6),
    ];
    let mut project = Project::empty("Shield Raspberry Pi 4 — GPIO17 e indicadores de alimentação");
    insert_device(
        &mut project,
        ids[0],
        "J1",
        device,
        header_fp,
        Point::new(-24, 0),
    );
    insert_component(
        &mut project,
        ids[1],
        "R1",
        ComponentKind::Resistor,
        1000.0,
        Point::new(0, -10),
        resistor_fp.clone(),
    );
    insert_component(
        &mut project,
        ids[2],
        "D1",
        ComponentKind::Led,
        2.0,
        Point::new(10, -10),
        led_fp.clone(),
    );
    insert_component(
        &mut project,
        ids[3],
        "R2",
        ComponentKind::Resistor,
        680.0,
        Point::new(0, 10),
        resistor_fp,
    );
    insert_component(
        &mut project,
        ids[4],
        "D2",
        ComponentKind::Led,
        2.0,
        Point::new(10, 10),
        led_fp,
    );
    insert_component(
        &mut project,
        ids[5],
        "TP_GPIO17",
        ComponentKind::Connector,
        0.0,
        Point::new(22, 0),
        tp_fp,
    );
    let ground_id = ComponentId(7);
    project.components.insert(
        ground_id,
        Component {
            id: ground_id,
            reference: "GND1".into(),
            kind: ComponentKind::Ground,
            value: 0.0,
            position: Point::new(-20, 28),
            rotation: 0,
            imported: None,
            device: None,
            footprint: None,
            parameters: Default::default(),
        },
    );
    project.next_id = 8;
    for index in 0..40u16 {
        let name = device_pin_name(&project, ids[0], index);
        let net = pi4_net_name(index, &name);
        add_component_label(&mut project, ids[0], index, &net);
    }
    for (component, pin, name) in [
        (ids[1], 0, "5V"),
        (ids[1], 1, "LED_5V_A"),
        (ids[2], 0, "LED_5V_A"),
        (ids[2], 1, "GND"),
        (ids[3], 0, "3V3"),
        (ids[3], 1, "LED_3V3_A"),
        (ids[4], 0, "LED_3V3_A"),
        (ids[4], 1, "GND"),
        (ids[5], 0, "P11_GPIO17"),
    ] {
        add_component_label(&mut project, component, pin, name);
    }
    add_component_label(&mut project, ground_id, 0, "GND");
    let j1_place = Placement {
        position: NmPoint::new(8_370_000, 5_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let r1_place = Placement {
        position: NmPoint::new(66_000_000, 18_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let d1_place = Placement {
        position: NmPoint::new(74_000_000, 18_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let r2_place = Placement {
        position: NmPoint::new(66_000_000, 36_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let d2_place = Placement {
        position: NmPoint::new(74_000_000, 36_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let tp_place = Placement {
        position: NmPoint::new(77_000_000, 47_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let mut board = Board {
        width_nm: 85 * NM_PER_MM,
        height_nm: 56 * NM_PER_MM,
        corner_radius_nm: 3 * NM_PER_MM,
        thickness_nm: 1_600_000,
        clearance_nm: 250_000,
        edge_clearance_nm: 500_000,
        placements: BTreeMap::new(),
        tracks: BTreeMap::new(),
        vias: BTreeMap::new(),
        holes: BTreeMap::new(),
        net_classes: BTreeMap::new(),
    };
    for (id, placement) in [
        (ids[0], j1_place),
        (ids[1], r1_place),
        (ids[2], d1_place),
        (ids[3], r2_place),
        (ids[4], d2_place),
        (ids[5], tp_place),
    ] {
        board.placements.insert(id, placement);
    }
    for (id, position) in [
        (1, (3_500_000, 3_500_000)),
        (2, (61_500_000, 3_500_000)),
        (3, (3_500_000, 52_500_000)),
        (4, (61_500_000, 52_500_000)),
    ] {
        board.holes.insert(
            id,
            MountingHole {
                id,
                position: NmPoint::new(position.0, position.1),
                drill_nm: 2_750_000,
            },
        );
    }
    project.board = Some(Box::new(board));
    let mut engine = Engine::new(project)?;
    let route_config = eda_pcb::AutorouteConfig {
        grid_nm: 500_000,
        max_expansions_per_connection: 800_000,
        max_total_expansions: 4_000_000,
        optimize_routes: false,
        ..Default::default()
    };
    let plan = eda_pcb::autoroute(engine.project(), &route_config)?;
    if !plan.failures.is_empty() {
        return Err(trf!(
            "O autorroteamento do protótipo deixou {count} conexões sem rota.",
            count = plan.failures.len()
        )
        .into());
    }
    engine.execute(Command::Batch(plan.commands))?;
    Ok(engine.project().clone())
}

/// Cria uma base mecânica HAT com o conector GPIO e os quatro furos oficiais.
/// O template não inclui EEPROM, componentes funcionais nem validação elétrica HAT+.
pub fn raspberry_pi_hat_project() -> Result<Project, Box<dyn Error>> {
    let (device, footprint) = eda_library::catalog()?.definition("RaspberryPi4B_GPIO")?;
    let mut board = Board {
        width_nm: 65 * NM_PER_MM,
        height_nm: 56 * NM_PER_MM,
        corner_radius_nm: 3 * NM_PER_MM,
        thickness_nm: 1_600_000,
        clearance_nm: 250_000,
        edge_clearance_nm: 500_000,
        placements: BTreeMap::new(),
        tracks: BTreeMap::new(),
        vias: BTreeMap::new(),
        holes: BTreeMap::new(),
        net_classes: BTreeMap::new(),
    };
    for (id, x_nm, y_nm) in [
        (1, 3_500_000, 3_500_000),
        (2, 61_500_000, 3_500_000),
        (3, 3_500_000, 52_500_000),
        (4, 61_500_000, 52_500_000),
    ] {
        board.holes.insert(
            id,
            MountingHole {
                id,
                position: NmPoint::new(x_nm, y_nm),
                drill_nm: 2_750_000,
            },
        );
    }
    let mut engine = Engine::new(Project::empty("Base mecânica Raspberry Pi HAT — 40 pinos"))?;
    engine.execute(Command::Batch(vec![
        Command::PlaceDevice {
            device: Box::new(device),
            footprint: Box::new(footprint),
            position: Point::new(0, 0),
        },
        Command::SetBoard { board },
        Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: Placement {
                position: NmPoint::new(8_370_000, 5_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        },
    ]))?;
    Ok(engine.project().clone())
}

pub fn buck_pi4_12v_project() -> Result<Project, Box<dyn Error>> {
    let project = eda_library::buck_pi4_project()?;
    let netlist = eda_netlist::build_netlist(&project);
    let route = eda_pcb::autoroute(
        &project,
        &eda_pcb::AutorouteConfig {
            grid_nm: 250_000,
            max_expansions_per_connection: 900_000,
            max_total_expansions: 5_000_000,
            prioritize_short_connections: true,
            optimize_routes: false,
            ..Default::default()
        },
    )?;
    if !route.failures.is_empty() {
        let details = route
            .failures
            .iter()
            .map(|failure| {
                let name = netlist
                    .nets
                    .iter()
                    .find(|net| net.id.0 == failure.net)
                    .map(|net| net.name.as_str())
                    .unwrap_or("?");
                format!(
                    "{name} ({:.2},{:.2}) → ({:.2},{:.2}): {}",
                    failure.start.x as f64 / NM_PER_MM as f64,
                    failure.start.y as f64 / NM_PER_MM as f64,
                    failure.end.x as f64 / NM_PER_MM as f64,
                    failure.end.y as f64 / NM_PER_MM as f64,
                    eda_i18n::text(&failure.reason)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(trf!(
            "O autorroteamento do protótipo de potência deixou {count} conexões sem rota: {details}.",
            count = route.failures.len(),
            details = details
        )
        .into());
    }
    let mut engine = Engine::new(project)?;
    if !route.commands.is_empty() {
        engine.execute(Command::Batch(route.commands))?;
    }
    let report = eda_pcb::check(engine.project())?;
    if !report.airwires.is_empty() {
        return Err(trf!(
            "O projeto de potência ainda tem {count} conexões sem rota.",
            count = report.airwires.len()
        )
        .into());
    }
    let error_diagnostics: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == "error")
        .collect();
    if !error_diagnostics.is_empty() {
        let details = error_diagnostics
            .iter()
            .take(12)
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(trf!(
            "O DRC encontrou {count} erros no protótipo de potência: {details}.",
            count = error_diagnostics.len(),
            details = details
        )
        .into());
    }
    Ok(engine.project().clone())
}

pub fn power_design_help() -> String {
    format!(
        "{}\n{}",
        tr!(
            "Uso: protocraft-cli power-design buck [--vin-v V] [--vout-v V] [--iout-a A] [--max-current-a A] [--switching-hz Hz] [--ripple-fraction F] [--efficiency E] [--inductance-uh µH] [--feedback-top-ohms Ω]\nCalcula valores de primeira ordem e imprime JSON; eficiência é apenas uma hipótese e não resultado medido."
        ),
        copper_design_help()
    )
}

fn copper_design_help() -> String {
    tr!(
        "Uso: power-design copper [--current-a A] [--copper-um µm] [--rise-c °C] [--ambient-c °C] [--length-mm mm] [--drop-v V] [--margin F] [--internal]\nEstima largura pelo ajuste histórico IPC-2221 e pela queda de tensão; não é análise térmica nem certificação de ampacidade."
    )
}

fn run_power_design(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.get(1).is_some_and(|mode| mode == "copper") {
        return run_copper_design(args);
    }
    let usage = power_design_help();
    if args.get(1).is_some_and(|argument| argument == "--help") {
        println!("{usage}");
        return Ok(());
    }
    if args.get(1).is_none_or(|mode| mode != "buck") {
        return Err(usage.into());
    }
    let mut spec = eda_library::BuckDesignSpec::default();
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        let value = args.get(index + 1).ok_or_else(|| usage.clone())?;
        let number: f64 = value
            .parse()
            .map_err(|_| trf!("Valor numérico inválido: {value}", value = value))?;
        match option {
            "--vin-v" => spec.input_voltage_v = number,
            "--vout-v" => spec.output_voltage_v = number,
            "--iout-a" => spec.output_current_a = number,
            "--max-current-a" => spec.regulator_current_limit_a = number,
            "--switching-hz" => spec.switching_frequency_hz = number,
            "--ripple-fraction" => spec.ripple_fraction_of_regulator_limit = number,
            "--efficiency" => spec.efficiency_assumption = number,
            "--inductance-uh" => spec.selected_inductance_h = number * 1e-6,
            "--feedback-top-ohms" => spec.feedback_top_ohms = number,
            _ => return Err(trf!("Opção desconhecida: {option}", option = option).into()),
        }
        index += 2;
    }
    let result = spec.calculate()?;
    json(&serde_json::json!({
        "calculation": "idealized_buck_first_order",
        "controller_reference": "LM61460",
        "design_spec": spec,
        "results": result,
    }))
}

fn run_copper_design(args: &[String]) -> Result<(), Box<dyn Error>> {
    let usage = tr!(
        "Uso: power-design copper [--current-a A] [--copper-um µm] [--rise-c °C] [--ambient-c °C] [--length-mm mm] [--drop-v V] [--margin F] [--internal]\nEstima largura pelo ajuste histórico IPC-2221 e pela queda de tensão; não é análise térmica nem certificação de ampacidade."
    );
    if args.get(2).is_some_and(|argument| argument == "--help") {
        println!("{usage}");
        return Ok(());
    }
    let mut spec = eda_library::CopperTraceSpec::default();
    let mut seen = HashSet::new();
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        if !seen.insert(option) {
            return Err(tr!("Opção de dimensionamento repetida.").into());
        }
        if option == "--internal" {
            spec.internal_layer = true;
            index += 1;
            continue;
        }
        let value = args.get(index + 1).ok_or_else(|| usage.clone())?;
        let number: f64 = value
            .parse()
            .map_err(|_| trf!("Valor numérico inválido: {value}", value = value))?;
        match option {
            "--current-a" => spec.current_a = number,
            "--copper-um" => spec.copper_thickness_um = number,
            "--rise-c" => spec.temperature_rise_c = number,
            "--ambient-c" => spec.ambient_temperature_c = number,
            "--length-mm" => spec.length_mm = number,
            "--drop-v" => spec.maximum_voltage_drop_v = number,
            "--margin" => spec.width_margin = number,
            _ => return Err(trf!("Opção desconhecida: {option}", option = option).into()),
        }
        index += 2;
    }
    json(&serde_json::json!({
        "calculation": "legacy_ipc2221_and_dc_voltage_drop_estimate",
        "design_spec": spec,
        "results": spec.calculate()?,
    }))
}

fn run_netclass(args: &[String]) -> Result<(), Box<dyn Error>> {
    let usage = tr!(
        "Uso: pcb-netclass <projeto> list | set --name <classe> --net <rede>... --width-mm N --clearance-mm N --via-diameter-mm N --via-drill-mm N --output <novo.protocraft> | remove --name <classe> --output <novo.protocraft>"
    );
    if args.get(1).is_some_and(|arg| arg == "--help") {
        println!("{usage}");
        return Ok(());
    }
    let input = args.get(1).ok_or_else(|| usage.clone())?;
    let action = args.get(2).ok_or_else(|| usage.clone())?;
    let mut project = eda_storage::load(input)?;
    if action == "list" {
        if args.len() != 3 {
            return Err(usage.into());
        }
        let board = project
            .board
            .as_deref()
            .ok_or_else(|| tr!("O projeto não tem PCB."))?;
        return json(&serde_json::json!({
            "format_version": project.format_version,
            "classes": board.net_classes
        }));
    }
    if !matches!(action.as_str(), "set" | "remove") {
        return Err(usage.into());
    }
    let mut board = *project
        .board
        .take()
        .ok_or_else(|| tr!("O projeto não tem PCB."))?;
    let mut name: Option<String> = None;
    let mut nets = BTreeSet::new();
    let mut width = None;
    let mut clearance = None;
    let mut via_diameter = None;
    let mut via_drill = None;
    let mut escape_width = None;
    let mut escape_length = None;
    let mut escape_via_diameter = None;
    let mut escape_via_drill = None;
    let mut output: Option<&str> = None;
    let mut seen = HashSet::new();
    let mut index = 3;
    while index < args.len() {
        let option = args[index].as_str();
        if option != "--net" && !seen.insert(option) {
            return Err(tr!("Opção repetida em pcb-netclass.").into());
        }
        match option {
            "--name" => {
                name = Some(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or_else(|| usage.clone())?
                        .clone(),
                );
            }
            "--net" => {
                let net = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| usage.clone())?
                    .clone();
                if net.trim().is_empty() || !nets.insert(net) {
                    return Err(tr!("Rede vazia ou repetida em pcb-netclass.").into());
                }
            }
            "--width-mm" => {
                width = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.05,
                    10.0,
                )?);
            }
            "--clearance-mm" => {
                clearance = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.0,
                    10.0,
                )?);
            }
            "--via-diameter-mm" => {
                via_diameter = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.3,
                    20.0,
                )?);
            }
            "--via-drill-mm" => {
                via_drill = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.1,
                    19.9,
                )?);
            }
            "--output" => {
                output = Some(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .map(String::as_str)
                        .ok_or_else(|| usage.clone())?,
                );
            }
            "--escape-width-mm" => {
                escape_width = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.05,
                    10.0,
                )?)
            }
            "--escape-length-mm" => {
                escape_length = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.05,
                    2.0,
                )?)
            }
            "--escape-via-diameter-mm" => {
                escape_via_diameter = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.3,
                    20.0,
                )?)
            }
            "--escape-via-drill-mm" => {
                escape_via_drill = Some(parse_rule_mm(
                    args.get(index + 1).ok_or_else(|| usage.clone())?,
                    0.1,
                    19.9,
                )?)
            }
            _ => return Err(usage.into()),
        }
        index += 2;
    }

    let name = name.ok_or_else(|| usage.clone())?;
    let destination = Path::new(output.ok_or_else(|| usage.clone())?);
    if destination
        .extension()
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("protocraft"))
    {
        return Err(tr!("A saída deve usar a extensão .protocraft.").into());
    }
    if same_path(
        &fs::canonicalize(input)?,
        &resolved_destination(destination)?,
    ) {
        return Err(tr!("A saída deve ser diferente do projeto original.").into());
    }
    if destination.exists() {
        return Err(tr!("O arquivo de saída já existe; escolha outro nome.").into());
    }

    if action == "remove" {
        if !nets.is_empty()
            || width.is_some()
            || clearance.is_some()
            || via_diameter.is_some()
            || via_drill.is_some()
            || escape_width.is_some()
            || escape_length.is_some()
            || escape_via_diameter.is_some()
            || escape_via_drill.is_some()
        {
            return Err(usage.into());
        }
        if board.net_classes.remove(&name).is_none() {
            return Err(tr!("Classe de rede não encontrada.").into());
        }
    } else {
        if nets.is_empty() {
            return Err(usage.into());
        }
        let width = width.ok_or_else(|| usage.clone())?;
        let clearance = clearance.ok_or_else(|| usage.clone())?;
        let via_diameter = via_diameter.ok_or_else(|| usage.clone())?;
        let via_drill = via_drill.ok_or_else(|| usage.clone())?;
        let existing_nets: BTreeSet<_> = eda_netlist::build_netlist(&project)
            .nets
            .into_iter()
            .map(|net| net.name)
            .collect();
        if nets.iter().any(|net| !existing_nets.contains(net)) {
            return Err(tr!("Uma rede informada não existe no netlist atual.").into());
        }
        let pad_escape = match (
            escape_width,
            escape_length,
            escape_via_diameter,
            escape_via_drill,
        ) {
            (None, None, None, None) => None,
            (
                Some(track_width_nm),
                Some(max_length_nm),
                Some(via_diameter_nm),
                Some(via_drill_nm),
            ) => Some(eda_core::PadEscapeRule {
                track_width_nm,
                max_length_nm,
                via_diameter_nm,
                via_drill_nm,
            }),
            _ => return Err(tr!("Informe todos os quatro parâmetros de escape de pads.").into()),
        };
        board.net_classes.insert(
            name.clone(),
            NetClass {
                net_names: nets,
                track_width_nm: width,
                clearance_nm: clearance,
                via_diameter_nm: via_diameter,
                via_drill_nm: via_drill,
                pad_escape,
            },
        );
    }
    project.board = Some(Box::new(board));
    project.validate()?;
    eda_storage::create_new(destination, &project)?;
    json(&serde_json::json!({
        "output": destination,
        "class": name,
        "operation": action,
        "format_version": project.format_version
    }))
}

fn parse_rule_mm(value: &str, minimum: f64, maximum: f64) -> Result<i64, Box<dyn Error>> {
    let millimeters: f64 = value.parse()?;
    if !millimeters.is_finite() || !(minimum..=maximum).contains(&millimeters) {
        return Err(trf!(
            "O valor precisa estar entre {minimum} e {maximum} mm.",
            minimum = minimum,
            maximum = maximum
        )
        .into());
    }
    Ok((millimeters * NM_PER_MM as f64).round() as i64)
}

fn pi4_net_name(pin: u16, name: &str) -> String {
    match name {
        "GND" if pin == 5 => "GND".into(),
        "5V" if pin == 1 => "5V".into(),
        "3V3" if pin == 0 => "3V3".into(),
        _ => format!("P{}_{}", pin + 1, name.replace('/', "_")),
    }
}
fn device_pin_name(project: &Project, id: ComponentId, index: u16) -> String {
    project.components[&id]
        .device
        .as_ref()
        .and_then(|d| d.pins.get(index as usize))
        .map(|p| p.name.clone())
        .unwrap_or_default()
}
fn add_component_label(project: &mut Project, id: ComponentId, pin: u16, name: &str) {
    if let Some(position) = project
        .components
        .get(&id)
        .and_then(|c| c.pins().get(pin as usize).map(|p| p.position))
    {
        let label = LabelId(project.next_id);
        project.next_id += 1;
        project.labels.insert(
            label,
            eda_core::NetLabel {
                id: label,
                position,
                name: name.into(),
            },
        );
    }
}
fn insert_device(
    project: &mut Project,
    id: ComponentId,
    reference: &str,
    device: DeviceDefinition,
    footprint: Footprint,
    position: Point,
) {
    project.components.insert(
        id,
        Component {
            id,
            reference: reference.into(),
            kind: ComponentKind::Device,
            value: 0.0,
            position,
            rotation: 0,
            imported: None,
            device: Some(Box::new(device)),
            footprint: Some(Box::new(footprint)),
            parameters: Default::default(),
        },
    );
    project.next_id = project.next_id.max(id.0 + 1);
}
fn insert_component(
    project: &mut Project,
    id: ComponentId,
    reference: &str,
    kind: ComponentKind,
    value: f64,
    position: Point,
    footprint: Footprint,
) {
    project.components.insert(
        id,
        Component {
            id,
            reference: reference.into(),
            kind,
            value,
            position,
            rotation: 0,
            imported: None,
            device: None,
            footprint: Some(Box::new(footprint)),
            parameters: Default::default(),
        },
    );
    project.next_id = project.next_id.max(id.0 + 1);
}
fn two_pad_footprint(
    id: &str,
    description: &str,
    spacing: i64,
    drill: i64,
    source: DataSource,
) -> Footprint {
    let pad = |number: &str, x: i64| Pad {
        number: number.into(),
        position: NmPoint::new(x, 0),
        size: NmPoint::new(drill + 800_000, drill + 800_000),
        rotation_mdeg: 0,
        shape: PadShape::Circle,
        technology: PadTechnology::ThroughHole,
        drill: Some(NmPoint::new(drill, drill)),
        layers: vec!["*.Cu".into(), "F.Mask".into()],
        roundrect_ratio_ppm: 0,
        mask_margin_nm: 0,
        paste_margin_nm: 0,
        paste_ratio_ppm: 0,
        polygons: vec![],
    };
    Footprint {
        id: id.into(),
        description: description.into(),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-spacing / 2 - 700_000, -1_500_000),
            max: NmPoint::new(spacing / 2 + 700_000, 1_500_000),
        },
        pads: vec![pad("1", -spacing / 2), pad("2", spacing / 2)],
        lines: vec![
            FootprintLine {
                start: NmPoint::new(-spacing / 2, -800_000),
                end: NmPoint::new(spacing / 2, -800_000),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(spacing / 2, -800_000),
                end: NmPoint::new(spacing / 2, 800_000),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(spacing / 2, 800_000),
                end: NmPoint::new(-spacing / 2, 800_000),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(-spacing / 2, 800_000),
                end: NmPoint::new(-spacing / 2, -800_000),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
        ],
        keepouts: vec![],
        source,
    }
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let cmd = &args[0];
    if cmd == "pcb-assembly" && args.get(1).is_some_and(|a| a == "--help") {
        println!(
            "{}",
            tr!(
                "Uso: protocraft-cli pcb-assembly <projeto> [--format json|bom-csv|placements-csv] [--output arquivo-novo]\nAs posições usam nanômetros e a rotação do projeto; confira a convenção do fabricante."
            )
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--help") {
        println!(
            "{}",
            tr!(
                "devices [--filter texto] [--limit N] [--offset N]\ndevice <modelo>\ndevice-project <modelo> --output arquivo.protocraft\nraspberry-pi-4-shield --output arquivo.protocraft\npcb-info <projeto>\npcb-check <projeto>\npcb-route <projeto> --output novo.protocraft [--grid-mm 0.25] [--track-width-mm 0.25] [--max-expansions 600000] [--no-optimize]\npcb-export <projeto> --format kicad|svg --output arquivo\npcb-manufacture <projeto> --output pasta-nova --kicad caminho/kicad-cli.exe [--preset jlcpcb|standard]\nPreset padrão: jlcpcb. Produz JLCPCB.zip, RS-274X/Protel, mm, 1:1, furação absoluta Excellon.\nCoordenadas físicas em nanômetros; 1 mm = 1.000.000 nm. Catálogo físico, sem emulação de firmware."
            )
        );
        return Ok(());
    }
    if cmd == "pcb-netclass" {
        return run_netclass(args);
    }
    if cmd == "power-design" {
        return run_power_design(args);
    }
    if cmd == "buck-pi4-12v" {
        if args.len() != 3 || args[1] != "--output" {
            return Err(
                tr!("Uso: protocraft-cli buck-pi4-12v --output <arquivo.protocraft>").into(),
            );
        }
        let path = Path::new(&args[2]);
        if path
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("protocraft"))
        {
            return Err(tr!("Use a extensão .protocraft").into());
        }
        if path.exists() {
            return Err(trf!(
                "O arquivo já existe; escolha outro caminho para preservar os dados: {path}",
                path = path.display()
            )
            .into());
        }
        let project = buck_pi4_12v_project()?;
        let report = eda_pcb::check(&project)?;
        eda_storage::create_new(path, &project)?;
        return json(&serde_json::json!({
            "output": path,
            "controller": "LM61460",
            "input_voltage_v": 12.0,
            "output_voltage_nominal_v": eda_library::BuckDesignSpec::default().calculate()?.output_voltage_nominal_v,
            "output_current_a": 3.0,
            "board_mm": {"width": 65.0, "height": 56.5, "corner_radius": 3.0},
            "mounting_holes": 4,
            "track_count": project.board.as_ref().map(|board| board.tracks.len()).unwrap_or(0),
            "minimum_power_track_width_mm": 0.8,
            "airwires": report.airwires.len(),
            "drc_errors": report.diagnostics.iter().filter(|diagnostic| diagnostic.severity == "error").count(),
            "hat_compliance": false,
            "format_version": eda_core::FORMAT_VERSION,
        }));
    }
    if cmd == "raspberry-pi-4-shield" {
        if args.len() != 3 || args[1] != "--output" {
            return Err(tr!("Uso: raspberry-pi-4-shield --output arquivo.protocraft").into());
        }
        let path = Path::new(&args[2]);
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("protocraft"))
        {
            return Err(tr!("Use a extensão .protocraft").into());
        }
        if path.exists() {
            return Err(trf!(
                "O arquivo já existe; escolha outro caminho para preservar os dados: {path}",
                path = path.display()
            )
            .into());
        }
        let project = raspberry_pi_4_shield_project()?;
        eda_storage::create_new(path, &project)?;
        return json(
            &serde_json::json!({"output":path,"device":"RaspberryPi4B_GPIO","board":{"width_mm":85,"height_mm":56,"corner_radius_mm":3},"gpio_pins":40,"mounting_holes":4,"format_version":eda_core::FORMAT_VERSION}),
        );
    }
    if cmd == "raspberry-pi-hat" {
        if args.len() != 3 || args[1] != "--output" {
            return Err(tr!("Uso: raspberry-pi-hat --output arquivo.protocraft").into());
        }
        let path = Path::new(&args[2]);
        if path
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("protocraft"))
        {
            return Err(tr!("Use a extensão .protocraft").into());
        }
        if path.exists() {
            return Err(trf!(
                "O arquivo já existe; escolha outro caminho para preservar os dados: {path}",
                path = path.display()
            )
            .into());
        }
        let project = raspberry_pi_hat_project()?;
        eda_storage::create_new(path, &project)?;
        return json(&serde_json::json!({
            "output": path,
            "device": "RaspberryPi4B_GPIO",
            "standard": "Raspberry Pi HAT mechanical outline",
            "hat_compliance": false,
            "board": {"width_mm": 65, "height_mm": 56, "corner_radius_mm": 3},
            "gpio_pins": 40,
            "mounting_holes": 4,
            "format_version": eda_core::FORMAT_VERSION
        }));
    }
    if cmd == "devices" {
        let mut filter = "";
        let mut limit = 100usize;
        let mut offset = 0usize;
        let mut i = 1;
        while i < args.len() {
            let value = args.get(i + 1).ok_or("Opção sem valor")?;
            match args[i].as_str() {
                "--filter" => filter = value,
                "--limit" => limit = value.parse()?,
                "--offset" => offset = value.parse()?,
                _ => return Err("Opção desconhecida".into()),
            }
            i += 2;
        }
        if limit == 0 || limit > 5000 {
            return Err("Limite deve estar entre 1 e 5000".into());
        }
        let c = eda_library::catalog()?;
        let matches = c.search(filter);
        let rows:Vec<_>=matches.iter().skip(offset).take(limit).map(|d|{let f=&c.footprints[&d.footprint];serde_json::json!({"id":d.id,"family":d.family,"manufacturer":d.manufacturer,"form":d.form,"pins":c.symbols[&d.symbol].len(),"footprint":d.footprint,"body_mm":{"width":f.body_bounds.width() as f64/1e6,"height":f.body_bounds.height() as f64/1e6},"source":d.source})}).collect();
        return json(&serde_json::json!({"total":matches.len(),"offset":offset,"devices":rows}));
    }
    let input = args.get(1).ok_or("Informe o modelo ou projeto")?;
    if cmd == "device" {
        if args.len() != 2 {
            return Err("Informe somente o modelo".into());
        }
        let (definition, footprint) = eda_library::catalog()?.definition(input)?;
        return json(
            &serde_json::json!({"device":definition,"footprint":footprint,"firmware_simulation":false}),
        );
    }
    if cmd == "device-project" {
        if args.len() != 4 || args[2] != "--output" {
            return Err("Uso: device-project <modelo> --output arquivo.protocraft".into());
        }
        let path = Path::new(&args[3]);
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("protocraft"))
        {
            return Err("Use a extensão .protocraft".into());
        }
        let c = eda_library::catalog()?;
        let mut engine = Engine::new(Project::empty(format!(
            "{} — projeto físico",
            c.find(input)?.id
        )))?;
        engine.execute(Command::Batch(vec![
            c.place_command(input, Point::new(0, 0))?,
            Command::SetBoard {
                board: Board::default(),
            },
            Command::PlaceOnBoard {
                id: ComponentId(1),
                placement: Placement {
                    position: NmPoint::new(50 * NM_PER_MM, 40 * NM_PER_MM),
                    rotation: 0,
                    side: Default::default(),
                },
            },
        ]))?;
        eda_storage::save(path, engine.project())?;
        return json(
            &serde_json::json!({"output":path,"device":c.find(input)?.id,"format_version":eda_core::FORMAT_VERSION}),
        );
    }
    let project = eda_storage::load(input)?;
    if cmd == "pcb-assembly" {
        let mut format = None;
        let mut output = None;
        let mut index = 2;
        while index < args.len() {
            let value = args.get(index + 1).ok_or("Opção sem valor")?;
            match args[index].as_str() {
                "--format" if format.is_none() => format = Some(value.as_str()),
                "--output" if output.is_none() => output = Some(value),
                _ => return Err("Opção desconhecida ou repetida".into()),
            }
            index += 2;
        }
        let report = eda_pcb::assembly::assembly_report(&project);
        let contents = match format.unwrap_or("json") {
            "json" => serde_json::to_string_pretty(&report)?,
            "bom-csv" => eda_pcb::assembly::bom_csv(&report),
            "placements-csv" => eda_pcb::assembly::placements_csv(&report),
            _ => return Err(tr!("Use --format json, bom-csv ou placements-csv.").into()),
        };
        if let Some(output) = output {
            let destination = Path::new(output);
            if same_path(
                &fs::canonicalize(input)?,
                &resolved_destination(destination)?,
            ) {
                return Err("A exportação deve preservar o projeto de entrada".into());
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            return json(
                &serde_json::json!({"output": destination, "format": format.unwrap_or("json")}),
            );
        }
        print!("{contents}");
        return Ok(());
    }
    if cmd == "pcb-info" {
        if args.len() != 2 {
            return Err("Opções inesperadas".into());
        }
        return json(
            &serde_json::json!({"board":project.board,"pin_map":eda_pcb::pin_map(&project)?,"routing_metrics":eda_pcb::metrics::routing_metrics(&project)?}),
        );
    }
    if cmd == "pcb-check" {
        if args.len() != 2 {
            return Err("Opções inesperadas".into());
        }
        let report = eda_pcb::check(&project)?;
        json(&report)?;
        if report.errors() > 0 {
            return Err("DRC encontrou violações; consulte os diagnósticos".into());
        }
        return Ok(());
    }
    if cmd == "pcb-route" {
        let mut output = None;
        let mut config = eda_pcb::AutorouteConfig::default();
        let mut seen = std::collections::HashSet::new();
        let mut i = 2;
        while i < args.len() {
            if !seen.insert(args[i].as_str()) {
                return Err("Opção desconhecida ou repetida em pcb-route".into());
            }
            match args[i].as_str() {
                "--output" if output.is_none() => {
                    output = Some(args.get(i + 1).ok_or("Informe o arquivo de saída")?)
                }
                "--grid-mm" => {
                    config.grid_nm = parse_mm_nm(args.get(i + 1).ok_or("Informe a grade")?)?;
                }
                "--track-width-mm" => {
                    config.track_width_nm =
                        parse_mm_nm(args.get(i + 1).ok_or("Informe a largura")?)?;
                }
                "--max-expansions" => {
                    let budget: usize = args.get(i + 1).ok_or("Informe o orçamento")?.parse()?;
                    config.max_total_expansions = budget;
                    config.max_expansions_per_connection =
                        config.max_expansions_per_connection.min(budget.max(1));
                }
                "--passes" => {
                    config.max_routing_passes = args
                        .get(i + 1)
                        .ok_or_else(|| tr!("Informe o número de tentativas de rota (1 a 8)."))?
                        .parse()?;
                }
                "--no-optimize" => {
                    config.optimize_routes = false;
                    i += 1;
                    continue;
                }
                _ => return Err("Opção desconhecida ou repetida em pcb-route".into()),
            }
            i += 2;
        }
        let output = output.ok_or("Informe --output novo.protocraft")?;
        let destination = Path::new(output);
        if destination
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("protocraft"))
        {
            return Err("A saída do autorroteamento deve usar .protocraft".into());
        }
        if same_path(
            &fs::canonicalize(input)?,
            &resolved_destination(destination)?,
        ) {
            return Err("O autorroteamento exige saída diferente do projeto original".into());
        }
        if destination.exists() {
            return Err("O arquivo de saída já existe; escolha outro nome.".into());
        }
        let plan = eda_pcb::autoroute(&project, &config)?;
        let mut engine = Engine::new(project)?;
        engine.execute(Command::Batch(plan.commands))?;
        eda_storage::create_new(destination, engine.project())?;
        return json(&serde_json::json!({
            "output": destination,
            "stats": plan.stats,
            "failures": plan.failures,
            "atomic": true,
            "format_version": eda_core::FORMAT_VERSION
        }));
    }
    if cmd == "pcb-manufacture" {
        use eda_storage::fabrication::ManufacturingPreset;
        let mut output = None;
        let mut tool = None;
        let mut preset = None;
        let mut dfm_limits = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--output" if output.is_none() => {
                    output = Some(args.get(i + 1).ok_or("Informe a pasta nova")?)
                }
                "--kicad" if tool.is_none() => {
                    tool = Some(args.get(i + 1).ok_or("Informe kicad-cli.exe")?)
                }
                "--preset" if preset.is_none() => {
                    preset = Some(match args.get(i + 1).map(String::as_str) {
                        Some("jlcpcb") => ManufacturingPreset::Jlcpcb,
                        Some("standard") => ManufacturingPreset::Standard,
                        _ => return Err("Use --preset jlcpcb ou standard".into()),
                    });
                }
                "--dfm-limits" if dfm_limits.is_none() => {
                    dfm_limits = Some(super::pcb_dfm_cli::read_limits(Path::new(
                        args.get(i + 1)
                            .ok_or_else(|| tr!("Informe o arquivo de limites DFM."))?,
                    ))?);
                }
                _ => return Err("Opção desconhecida ou repetida".into()),
            }
            i += 2;
        }
        let destination = Path::new(output.ok_or("Informe --output pasta-nova")?);
        let tool = Path::new(tool.ok_or("Informe --kicad caminho/kicad-cli.exe")?);
        let preset = preset.unwrap_or_default();
        let dfm_report = if let Some(limits) = &dfm_limits {
            let report = eda_pcb::dfm::check(&project, limits)?;
            if report.errors() > 0 || !report.complete {
                json(&report)?;
                return Err(
                    tr!("A revisão DFM encontrou pendências; consulte o relatório.").into(),
                );
            }
            Some(report)
        } else {
            None
        };
        let path = eda_storage::fabrication::generate_with_preset_and_limits(
            &project,
            destination,
            tool,
            preset,
            dfm_limits.as_ref(),
        )?;
        let archive = preset.archive_name().map(|name| path.join(name));
        return json(
            &serde_json::json!({"output":path,"preset":preset.id(),"archive":archive,"gerber":true,"excellon":true,"external_drc":true,"dfm_review":dfm_report.is_some()}),
        );
    }
    if cmd == "pcb-export" {
        let mut format = None;
        let mut output = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--format" if format.is_none() => {
                    format = Some(args.get(i + 1).ok_or("Informe o formato")?.as_str())
                }
                "--output" if output.is_none() => {
                    output = Some(args.get(i + 1).ok_or("Informe a saída")?)
                }
                _ => return Err("Opção desconhecida ou repetida".into()),
            }
            i += 2;
        }
        let output = output.ok_or("Informe --output")?;
        if same_path(
            &fs::canonicalize(input)?,
            &resolved_destination(Path::new(output))?,
        ) {
            return Err("A exportação deve preservar o projeto de entrada".into());
        }
        let text = match format.ok_or("Informe --format kicad ou svg")? {
            "kicad" => eda_pcb::export_kicad(&project)?,
            "svg" => eda_pcb::export_svg(&project)?,
            _ => return Err("Formato deve ser kicad ou svg".into()),
        };
        fs::write(output, text)?;
        println!("{}", trf!("PCB exportada: {path}", path = output));
        return Ok(());
    }
    Err("Comando físico desconhecido".into())
}

fn parse_mm_nm(value: &str) -> Result<i64, Box<dyn Error>> {
    let millimeters: f64 = value.parse()?;
    if !millimeters.is_finite() || !(0.05..=10.0).contains(&millimeters) {
        return Err("Use uma distância entre 0,05 e 10 mm".into());
    }
    Ok((millimeters * NM_PER_MM as f64).round() as i64)
}
