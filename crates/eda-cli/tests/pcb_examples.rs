use std::process::Command;

#[test]
fn examples_command_exports_native_pcb_gallery_without_overwriting() {
    let directory = tempfile::tempdir().expect("pasta temporária");
    let output = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .arg("examples")
        .arg(directory.path())
        .output()
        .expect("CLI executável");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for id in [
        "pcb-trilhas-furos",
        "pcb-555-led-1hz",
        "pcb-stm32-esp32",
        "pcb-raspberry-pi-4-shield",
        "pcb-pi4-12v-buck",
    ] {
        let path = directory.path().join(format!("{id}.protocraft"));
        let project = eda_storage::load(&path).expect("projeto nativo gerado pela CLI");
        assert!(project.board.is_some(), "{id} deve conter PCB");
        assert!(
            !project
                .board
                .as_ref()
                .expect("PCB presente")
                .net_classes
                .is_empty(),
            "{id} deve demonstrar classes de rede"
        );
        let check = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .arg("pcb-check")
            .arg(&path)
            .output()
            .expect("comando DRC da CLI");
        if id == "pcb-stm32-esp32" {
            let report = String::from_utf8_lossy(&check.stdout);
            assert!(
                !check.status.success(),
                "o layout de demonstração deve manter airwires"
            );
            assert!(report.contains("UNROUTED"), "{report}");
            assert!(!report.contains("NETCLASS_UNUSED_NET"), "{report}");
        } else {
            assert!(
                check.status.success(),
                "DRC do exemplo {id}: {}\n{}",
                String::from_utf8_lossy(&check.stdout),
                String::from_utf8_lossy(&check.stderr)
            );
        }
    }

    let source = directory.path().join("pcb-trilhas-furos.protocraft");
    let updated = directory.path().join("pcb-netclass-updated.protocraft");
    let set = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args([
            "pcb-netclass",
            source.to_str().expect("caminho UTF-8"),
            "set",
            "--name",
            "SINAL",
            "--net",
            "SIGNAL",
            "--width-mm",
            "0.40",
            "--clearance-mm",
            "0.25",
            "--via-diameter-mm",
            "0.60",
            "--via-drill-mm",
            "0.30",
            "--output",
            updated.to_str().expect("caminho UTF-8"),
        ])
        .output()
        .expect("CLI para gravar classe de rede");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );
    let saved = eda_storage::load(&updated).expect("projeto atualizado pela CLI");
    assert_eq!(
        saved.board.unwrap().net_classes["SINAL"].track_width_nm,
        400_000
    );
    let listed = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args([
            "pcb-netclass",
            updated.to_str().expect("caminho UTF-8"),
            "list",
        ])
        .output()
        .expect("CLI para listar classes");
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains("SINAL"));

    let removed = directory.path().join("pcb-netclass-removed.protocraft");
    let remove = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args([
            "pcb-netclass",
            updated.to_str().expect("caminho UTF-8"),
            "remove",
            "--name",
            "SINAL",
            "--output",
            removed.to_str().expect("caminho UTF-8"),
        ])
        .output()
        .expect("CLI para remover classe");
    assert!(remove.status.success());
    assert!(
        eda_storage::load(removed)
            .expect("projeto sem classe")
            .board
            .unwrap()
            .net_classes
            .is_empty()
    );

    let second_run = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .arg("examples")
        .arg(directory.path())
        .output()
        .expect("CLI executável");
    assert!(!second_run.status.success());
}
