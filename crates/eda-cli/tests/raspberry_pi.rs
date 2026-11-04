use eda_core::{ComponentId, NM_PER_MM};
use std::{collections::BTreeSet, path::Path, process::Command};

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn cli_in_language(args: &[&str], language: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .env("PROTOCRAFT_LANGUAGE", language)
        .args(args)
        .output()
        .unwrap()
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn raspberry_pi_4_shield_is_created_and_validated_from_cli() {
    let dir = tempfile::tempdir().unwrap();
    let project_path = dir.path().join("pi4-shield.protocraft");
    let created = cli(&["raspberry-pi-4-shield", "--output", path(&project_path)]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let project: eda_core::Project = eda_storage::load(&project_path).unwrap();
    project.validate().unwrap();

    let board = project.board.as_ref().unwrap();
    assert_eq!(
        (board.width_nm, board.height_nm),
        (85 * NM_PER_MM, 56 * NM_PER_MM)
    );
    assert_eq!(board.corner_radius_nm, 3 * NM_PER_MM);
    assert_eq!(board.holes.len(), 4);
    let holes: BTreeSet<_> = board
        .holes
        .values()
        .map(|hole| (hole.position.x, hole.position.y, hole.drill_nm))
        .collect();
    assert_eq!(
        holes,
        [
            (3_500_000, 3_500_000, 2_750_000),
            (61_500_000, 3_500_000, 2_750_000),
            (3_500_000, 52_500_000, 2_750_000),
            (61_500_000, 52_500_000, 2_750_000),
        ]
        .into_iter()
        .collect()
    );

    let header = &project.components[&ComponentId(1)];
    let device = header.device.as_ref().unwrap();
    assert_eq!(device.pins.len(), 40);
    let expected_pin_names = [
        "3V3",
        "5V",
        "GPIO2/SDA1",
        "5V",
        "GPIO3/SCL1",
        "GND",
        "GPIO4/GPCLK0",
        "GPIO14/TXD0",
        "GND",
        "GPIO15/RXD0",
        "GPIO17",
        "GPIO18/PWM0",
        "GPIO27",
        "GND",
        "GPIO22",
        "GPIO23",
        "3V3",
        "GPIO24",
        "GPIO10/SPI0_MOSI",
        "GND",
        "GPIO9/SPI0_MISO",
        "GPIO25",
        "GPIO11/SPI0_SCLK",
        "GPIO8/SPI0_CE0",
        "GND",
        "GPIO7/SPI0_CE1",
        "ID_SD",
        "ID_SC",
        "GPIO5",
        "GND",
        "GPIO6",
        "GPIO12/PWM0",
        "GPIO13/PWM1",
        "GND",
        "GPIO19/PCM_FS",
        "GPIO16",
        "GPIO26",
        "GPIO20/PCM_DIN",
        "GND",
        "GPIO21/PCM_DOUT",
    ];
    assert_eq!(
        device
            .pins
            .iter()
            .map(|pin| pin.name.as_str())
            .collect::<Vec<_>>(),
        expected_pin_names
    );
    let header_footprint = header.footprint.as_ref().unwrap();
    assert_eq!(header_footprint.pads.len(), 40);
    let placement = board.placements[&ComponentId(1)];
    assert_eq!(
        placement.position,
        eda_core::NmPoint::new(8_370_000, 5_000_000)
    );
    let pad1 = header_footprint
        .pads
        .iter()
        .find(|pad| pad.number == "1")
        .unwrap();
    let pad2 = header_footprint
        .pads
        .iter()
        .find(|pad| pad.number == "2")
        .unwrap();
    assert_eq!(pad1.position, eda_core::NmPoint::new(0, 0));
    assert_eq!(pad2.position, eda_core::NmPoint::new(0, 2_540_000));
    assert_eq!(project.components[&ComponentId(2)].value, 1000.0);
    assert_eq!(project.components[&ComponentId(4)].value, 680.0);
    assert!(project.components.contains_key(&ComponentId(6))); // furo de teste GPIO17

    let netlist = eda_netlist::build_netlist(&project);
    let net = |component, pin| {
        netlist
            .net_for_pin(eda_core::PinRef { component, pin })
            .unwrap()
    };
    assert_eq!(net(ComponentId(1), 1), net(ComponentId(2), 0)); // 5 V -> R1
    assert_eq!(net(ComponentId(2), 1), net(ComponentId(3), 0)); // R1 -> LED vermelho
    assert_eq!(net(ComponentId(3), 1), net(ComponentId(1), 5)); // LED -> GND
    assert_eq!(net(ComponentId(1), 0), net(ComponentId(4), 0)); // 3V3 -> R2
    assert_eq!(net(ComponentId(4), 1), net(ComponentId(5), 0)); // R2 -> LED verde
    assert_eq!(net(ComponentId(5), 1), net(ComponentId(1), 5)); // LED -> GND
    assert_eq!(net(ComponentId(1), 10), net(ComponentId(6), 0)); // GPIO17 -> test point
    assert!(!board.tracks.is_empty());

    let drc = cli(&["pcb-check", path(&project_path)]);
    assert!(
        drc.status.success(),
        "{}",
        String::from_utf8_lossy(&drc.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&drc.stdout).unwrap();
    assert_eq!(report["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(report["airwires"].as_array().unwrap().len(), 0);

    let kicad = dir.path().join("pi4-shield.kicad_pcb");
    let exported = cli(&[
        "pcb-export",
        path(&project_path),
        "--format",
        "kicad",
        "--output",
        path(&kicad),
    ]);
    assert!(exported.status.success());
    let kicad_text = std::fs::read_to_string(kicad).unwrap();
    assert!(kicad_text.contains("(gr_arc"));
    assert!(kicad_text.contains("Edge.Cuts"));
    assert!(kicad_text.trim_end().ends_with(')'));

    let svg = dir.path().join("pi4-shield.svg");
    let exported = cli(&[
        "pcb-export",
        path(&project_path),
        "--format",
        "svg",
        "--output",
        path(&svg),
    ]);
    assert!(exported.status.success());
    assert!(std::fs::read_to_string(svg).unwrap().contains("rx=\"3\""));

    let original = std::fs::read(&project_path).unwrap();
    assert!(
        !cli(&["raspberry-pi-4-shield", "--output", path(&project_path)])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&project_path).unwrap(), original);
}

#[test]
fn raspberry_pi_4_catalog_pinout_is_available_in_cli() {
    for args in [
        &["--help"][..],
        &["new", "--help"][..],
        &["raspberry-pi-4-shield", "--help"][..],
    ] {
        let help = cli(args);
        assert!(help.status.success());
        assert!(String::from_utf8_lossy(&help.stdout).contains("raspberry-pi-4-shield"));
    }
    let english_help = cli_in_language(&["raspberry-pi-4-shield", "--help"], "en");
    assert!(String::from_utf8_lossy(&english_help.stdout).contains("Creates the Raspberry Pi 4"));
    let portuguese_help = cli_in_language(&["raspberry-pi-4-shield", "--help"], "pt-BR");
    assert!(
        String::from_utf8_lossy(&portuguese_help.stdout).contains("Cria o shield Raspberry Pi 4")
    );
    let english_hat_help = cli_in_language(&["raspberry-pi-hat", "--help"], "en");
    assert!(String::from_utf8_lossy(&english_hat_help.stdout).contains("Creates a 65×56 mm"));
    let portuguese_hat_help = cli_in_language(&["raspberry-pi-hat", "--help"], "pt-BR");
    assert!(
        String::from_utf8_lossy(&portuguese_hat_help.stdout).contains("Cria uma base mecânica")
    );

    let templates = cli(&["templates"]);
    assert!(templates.status.success());
    let templates: serde_json::Value = serde_json::from_slice(&templates.stdout).unwrap();
    assert!(
        templates
            .as_array()
            .unwrap()
            .iter()
            .any(|template| { template["id"] == "raspberry-pi-4-shield" })
    );

    let listed = cli(&["devices", "--filter", "RaspberryPi4B_GPIO"]);
    assert!(listed.status.success());
    let rows: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(rows["total"], 1);
    assert_eq!(rows["devices"][0]["pins"], 40);

    let detail = cli(&["device", "RaspberryPi4B_GPIO"]);
    assert!(detail.status.success());
    let detail: serde_json::Value = serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(detail["device"]["pins"][10]["number"], "11");
    assert_eq!(detail["device"]["pins"][10]["name"], "GPIO17");

    let pico_list = cli(&["devices", "--filter", "RaspberryPiPico2W_GPIO40"]);
    assert!(pico_list.status.success());
    let pico_list: serde_json::Value = serde_json::from_slice(&pico_list.stdout).unwrap();
    assert_eq!(pico_list["total"], 1);
    assert_eq!(pico_list["devices"][0]["pins"], 40);
}

#[test]
fn new_template_creates_the_raspberry_pi_shield() {
    let dir = tempfile::tempdir().unwrap();
    let project_path = dir.path().join("template.protocraft");
    let result = cli(&[
        "new",
        "--template",
        "raspberry-pi-4-shield",
        "--output",
        path(&project_path),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let project: eda_core::Project = eda_storage::load(project_path).unwrap();
    assert_eq!(
        project.name,
        "Shield Raspberry Pi 4 — GPIO17 e indicadores de alimentação"
    );
    assert_eq!(project.board.unwrap().corner_radius_nm, 3 * NM_PER_MM);
}

#[test]
fn raspberry_pi_hat_mechanical_profile_and_other_pinouts_work_through_cli() {
    let dir = tempfile::tempdir().unwrap();
    let project_path = dir.path().join("pi-hat-base.protocraft");
    let created = cli(&["raspberry-pi-hat", "--output", path(&project_path)]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(metadata["standard"], "Raspberry Pi HAT mechanical outline");
    assert_eq!(metadata["hat_compliance"], false);

    let project: eda_core::Project = eda_storage::load(&project_path).unwrap();
    project.validate().unwrap();
    let board = project.board.as_ref().unwrap();
    assert_eq!(
        (board.width_nm, board.height_nm),
        (65 * NM_PER_MM, 56 * NM_PER_MM)
    );
    assert_eq!(board.corner_radius_nm, 3 * NM_PER_MM);
    assert_eq!(board.holes.len(), 4);
    let hole_centers: BTreeSet<_> = board
        .holes
        .values()
        .map(|hole| (hole.position.x, hole.position.y, hole.drill_nm))
        .collect();
    assert_eq!(
        hole_centers,
        [
            (3_500_000, 3_500_000, 2_750_000),
            (61_500_000, 3_500_000, 2_750_000),
            (3_500_000, 52_500_000, 2_750_000),
            (61_500_000, 52_500_000, 2_750_000),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(board.placements.len(), 1);
    assert_eq!(project.components[&ComponentId(1)].pins().len(), 40);
    let drc = cli(&["pcb-check", path(&project_path)]);
    assert!(
        drc.status.success(),
        "{}",
        String::from_utf8_lossy(&drc.stdout)
    );

    let templates = cli(&["templates"]);
    let templates: serde_json::Value = serde_json::from_slice(&templates.stdout).unwrap();
    assert!(
        templates
            .as_array()
            .unwrap()
            .iter()
            .any(|template| template["id"] == "raspberry-pi-hat")
    );
    let template_path = dir.path().join("hat-template.protocraft");
    let template_created = cli(&[
        "new",
        "--template",
        "raspberry-pi-hat",
        "--output",
        path(&template_path),
    ]);
    assert!(
        template_created.status.success(),
        "{}",
        String::from_utf8_lossy(&template_created.stderr)
    );
    let template_project: eda_core::Project = eda_storage::load(template_path).unwrap();
    assert_eq!(template_project.board.unwrap().width_nm, 65 * NM_PER_MM);

    let pico = cli(&["device", "Raspberry Pi Pico 2 W"]);
    assert!(
        pico.status.success(),
        "{}",
        String::from_utf8_lossy(&pico.stderr)
    );
    let pico: serde_json::Value = serde_json::from_slice(&pico.stdout).unwrap();
    assert_eq!(pico["device"]["pins"].as_array().unwrap().len(), 40);
    assert_eq!(pico["device"]["pins"][30]["name"], "GP26/ADC0");
    assert_eq!(pico["footprint"]["pads"][20]["position"]["x"], 17_780_000);

    let pico_project_path = dir.path().join("pico-project.protocraft");
    let pico_project_created = cli(&[
        "device-project",
        "Raspberry Pi Pico 2 W",
        "--output",
        path(&pico_project_path),
    ]);
    assert!(
        pico_project_created.status.success(),
        "{}",
        String::from_utf8_lossy(&pico_project_created.stderr)
    );
    let pico_project: eda_core::Project = eda_storage::load(pico_project_path).unwrap();
    assert_eq!(pico_project.components[&ComponentId(1)].pins().len(), 40);
    assert_eq!(pico_project.board.unwrap().placements.len(), 1);

    let legacy = cli(&["device", "RaspberryPi1BRev1_GPIO26"]);
    assert!(legacy.status.success());
    let legacy: serde_json::Value = serde_json::from_slice(&legacy.stdout).unwrap();
    assert_eq!(legacy["device"]["pins"].as_array().unwrap().len(), 26);
    assert_eq!(legacy["device"]["pins"][2]["name"], "GPIO0/SDA0");
}
