use eda_core::{ComponentId, NM_PER_MM, PinRef};
use std::process::Command;

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .expect("executável da CLI")
}

#[test]
fn power_design_calculator_returns_first_order_buck_numbers_as_json() {
    let result = cli(&["power-design", "buck"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(output["calculation"], "idealized_buck_first_order");
    assert_eq!(output["controller_reference"], "LM61460");
    assert_eq!(output["design_spec"]["input_voltage_v"], 12.0);
    assert_eq!(output["design_spec"]["output_current_a"], 3.0);
    assert!((output["results"]["output_power_w"].as_f64().unwrap() - 15.3).abs() < 1e-12);
    assert!(
        (output["results"]["output_voltage_nominal_v"]
            .as_f64()
            .unwrap()
            - 5.115)
            .abs()
            < 0.001
    );
    assert!(
        (output["results"]["minimum_inductance_h"].as_f64().unwrap() - 4.8875e-6).abs() < 1e-10
    );

    let invalid = cli(&["power-design", "buck", "--vin-v", "4", "--vout-v", "5.1"]);
    assert!(!invalid.status.success());
    let invalid_lm61460_range = cli(&["power-design", "buck", "--vin-v", "48"]);
    assert!(!invalid_lm61460_range.status.success());
}

#[test]
fn lm61460_is_searchable_and_exposes_exact_pin_to_pad_mapping() {
    let output = cli(&["device", "LM61460"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["device"]["manufacturer"], "Texas Instruments");
    assert_eq!(response["device"]["pins"].as_array().unwrap().len(), 14);
    assert_eq!(response["device"]["pins"][9]["name"], "SW");
    assert_eq!(response["device"]["pins"][9]["pads"][0], "10");
    assert_eq!(response["footprint"]["pads"].as_array().unwrap().len(), 14);
}

#[test]
fn routed_pi4_buck_template_has_power_nets_and_passes_cli_drc() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pi4-buck.protocraft");
    let path_text = path.to_str().unwrap();
    let created = cli(&["new", "--template", "buck-pi4-12v", "--output", path_text]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let project: eda_core::Project = eda_storage::load(&path).unwrap();
    project.validate().unwrap();
    let board = project.board.as_ref().unwrap();
    assert_eq!(
        (board.width_nm, board.height_nm),
        (65 * NM_PER_MM, 56_500_000)
    );
    assert_eq!(board.corner_radius_nm, 3 * NM_PER_MM);
    assert_eq!(board.holes.len(), 4);
    assert_eq!(project.components[&ComponentId(1)].pins().len(), 40);
    assert_eq!(
        project.components[&ComponentId(2)]
            .device
            .as_ref()
            .unwrap()
            .id,
        "LM61460"
    );
    assert!(board.tracks.len() >= 100);

    let netlist = eda_netlist::build_netlist(&project);
    let five_volt = netlist
        .net_for_pin(PinRef {
            component: ComponentId(1),
            pin: 1,
        })
        .unwrap();
    assert_eq!(
        five_volt,
        netlist
            .net_for_pin(PinRef {
                component: ComponentId(1),
                pin: 3
            })
            .unwrap()
    );
    assert_eq!(
        five_volt,
        netlist
            .net_for_pin(PinRef {
                component: ComponentId(4),
                pin: 1
            })
            .unwrap()
    );
    let power_track_widths: Vec<_> = board
        .tracks
        .values()
        .filter(|track| {
            netlist
                .net_for_pin(track.net)
                .is_some_and(|net| net == five_volt)
        })
        .map(|track| track.width_nm)
        .collect();
    assert!(!power_track_widths.is_empty());
    assert!(
        power_track_widths
            .iter()
            .filter(|width| **width >= 800_000)
            .count()
            > 5
    );
    assert_eq!(board.net_classes["OUTPUT_3A"].track_width_nm, 800_000);
    assert_eq!(
        board.net_classes["OUTPUT_3A"]
            .pad_escape
            .as_ref()
            .unwrap()
            .max_length_nm,
        2_000_000
    );
    for track in board.tracks.values().filter(|track| {
        netlist.net_for_pin(track.net) == Some(five_volt) && track.width_nm < 800_000
    }) {
        assert_eq!(track.net.component, ComponentId(2));
        assert!(
            (track.end.x - track.start.x)
                .abs()
                .max((track.end.y - track.start.y).abs())
                <= 2_000_000
        );
    }

    let drc = cli(&["pcb-check", path_text]);
    assert!(
        drc.status.success(),
        "{}",
        String::from_utf8_lossy(&drc.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&drc.stdout).unwrap();
    assert_eq!(report["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(report["airwires"].as_array().unwrap().len(), 0);
}

#[test]
fn copper_calculator_and_cli_pad_escape_configuration_are_reusable() {
    let result = cli(&["power-design", "copper"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!((output["results"]["selected_width_mm"].as_f64().unwrap() - 0.8).abs() < 1e-12);
    assert!(
        output["results"]["voltage_drop_estimate_v"]
            .as_f64()
            .unwrap()
            < 0.05
    );
    assert!(
        !cli(&["power-design", "copper", "--current-a", "NaN"])
            .status
            .success()
    );
    assert!(
        !cli(&[
            "power-design",
            "copper",
            "--current-a",
            "3",
            "--current-a",
            "2"
        ])
        .status
        .success()
    );

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.protocraft");
    let destination = directory.path().join("updated.protocraft");
    let project = eda_library::load_pcb_example("pcb-trilhas-furos").unwrap();
    eda_storage::create_new(&source, &project).unwrap();
    let result = cli(&[
        "pcb-netclass",
        source.to_str().unwrap(),
        "set",
        "--name",
        "SINAL",
        "--net",
        "SIGNAL",
        "--width-mm",
        "0.8",
        "--clearance-mm",
        "0.1",
        "--via-diameter-mm",
        "0.8",
        "--via-drill-mm",
        "0.4",
        "--escape-width-mm",
        "0.25",
        "--escape-length-mm",
        "1",
        "--escape-via-diameter-mm",
        "0.4",
        "--escape-via-drill-mm",
        "0.2",
        "--output",
        destination.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = eda_storage::load(&destination).unwrap();
    assert_eq!(
        saved.board.unwrap().net_classes["SINAL"]
            .pad_escape
            .as_ref()
            .unwrap()
            .track_width_nm,
        250_000
    );
}

#[test]
fn direct_buck_command_and_template_are_listed_in_cli() {
    let help = cli(&["buck-pi4-12v", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("buck-pi4-12v"));

    let templates = cli(&["templates"]);
    let templates: serde_json::Value = serde_json::from_slice(&templates.stdout).unwrap();
    assert!(
        templates
            .as_array()
            .unwrap()
            .iter()
            .any(|template| template["id"] == "buck-pi4-12v")
    );

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("direct.protocraft");
    let direct = cli(&["buck-pi4-12v", "--output", path.to_str().unwrap()]);
    assert!(
        direct.status.success(),
        "{}",
        String::from_utf8_lossy(&direct.stderr)
    );
}
