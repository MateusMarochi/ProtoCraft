use std::process::Command;

const CASES: &[(&str, &str)] = &[
    (
        "pcb-assembly",
        "Uso: protocraft-cli pcb-assembly <projeto> [--format json|bom-csv|placements-csv] [--output arquivo-novo]\nAs posições usam nanômetros e a rotação do projeto; confira a convenção do fabricante.",
    ),
    (
        "pcb-sync",
        "Uso: protocraft-cli pcb-sync <projeto.protocraft> --output <novo.protocraft>\nAdiciona footprints não posicionados e preserva o projeto original. Confira o DRC.",
    ),
    (
        "pcb-sync-report",
        "Uso: protocraft-cli pcb-sync-report <projeto.protocraft>\nInspeciona footprints e posicionamentos sem alterar o projeto.",
    ),
    (
        "pcb-move",
        "Uso: protocraft-cli pcb-move <projeto.protocraft> --component <id> --x-mm <x> --y-mm <y> --output <novo.protocraft>",
    ),
    (
        "pcb-cleanup",
        "Uso: protocraft-cli pcb-cleanup <projeto.protocraft> --output <novo.protocraft> [--merge-collinear]",
    ),
];

#[test]
fn pcb_help_and_errors_follow_system_language() {
    for (command, source) in CASES {
        let help = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .args([command, "--help"])
            .env_remove("PROTOCRAFT_LANGUAGE")
            .output()
            .unwrap();
        assert!(help.status.success(), "{command}");
        assert_eq!(
            String::from_utf8(help.stdout).unwrap().trim(),
            eda_i18n::text(source)
        );
        let error = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .arg(command)
            .env_remove("PROTOCRAFT_LANGUAGE")
            .output()
            .unwrap();
        assert!(!error.status.success(), "{command}");
        let stderr = String::from_utf8(error.stderr).unwrap();
        assert!(
            stderr.starts_with(&format!("{}:", eda_i18n::text("Erro"))),
            "{command}: {stderr}"
        );
    }
}

#[test]
fn pcb_help_follows_child_locale_in_both_languages() {
    for (locale, catalog) in [
        ("en", include_str!("../../../assets/i18n/en.json")),
        ("pt-BR", include_str!("../../../assets/i18n/pt-BR.json")),
    ] {
        let translations: serde_json::Value = serde_json::from_str(catalog).unwrap();
        for (command, source) in CASES {
            let expected = translations[source]
                .as_str()
                .unwrap_or_else(|| panic!("chave ausente: {command}"));
            let output = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
                .args([command, "--help"])
                .env("PROTOCRAFT_LANGUAGE", locale)
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                expected,
                "{command}, {locale}"
            );
            let error = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
                .arg(command)
                .env("PROTOCRAFT_LANGUAGE", locale)
                .output()
                .unwrap();
            assert!(!error.status.success());
            let stderr = String::from_utf8(error.stderr).unwrap();
            let prefix = translations["Erro"].as_str().unwrap();
            assert!(
                stderr.starts_with(&format!("{prefix}:")),
                "{command}, {locale}: {stderr}"
            );
            let error_key = match *command {
                "pcb-assembly" => "Informe o modelo ou projeto",
                "pcb-sync" => {
                    "Uso: protocraft-cli pcb-sync <projeto.protocraft> --output <novo.protocraft>"
                }
                "pcb-sync-report" => "Uso: protocraft-cli pcb-sync-report <projeto.protocraft>",
                "pcb-move" => {
                    "Uso: protocraft-cli pcb-move <projeto.protocraft> --component <id> --x-mm <x> --y-mm <y> --output <novo.protocraft>"
                }
                "pcb-cleanup" => {
                    "Uso: protocraft-cli pcb-cleanup <projeto.protocraft> --output <novo.protocraft> [--merge-collinear]"
                }
                _ => unreachable!(),
            };
            let expected_error = translations[error_key]
                .as_str()
                .unwrap_or_else(|| panic!("chave ausente: {error_key}"));
            assert!(
                stderr.contains(expected_error),
                "{command}, {locale}: {stderr}"
            );
        }
    }
    let invalid = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .arg("--help")
        .env("PROTOCRAFT_LANGUAGE", "invalid")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
}
