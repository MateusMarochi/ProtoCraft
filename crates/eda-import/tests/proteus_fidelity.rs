use eda_import::{
    ImportError,
    dsn::{DsnFormat, detect_dsn_format, parse_dsn},
    import_proteus,
};
use std::io::Write;
fn wire() -> Vec<u8> {
    let mut b =
        b"ISIS SCHEMATIC FILE\x1aISIS CIRCUIT FILE\x1aOBJECT DATA\0\x02\x7fWIRE\0\0\0".to_vec();
    b.extend(2u16.to_le_bytes());
    for n in [-300i32, 400, 500, 400] {
        b.extend(n.to_le_bytes());
    }
    b
}
#[test]
fn dsn_signature_distinguishes_pcb_routes_from_proteus_schematic() {
    assert_eq!(detect_dsn_format(&wire()), DsnFormat::ProteusIsis);
    for d in [
        b"(pcb board)".as_slice(),
        b"\xef\xbb\xbf ; comment\n ( pcb board)",
    ] {
        assert_eq!(detect_dsn_format(d), DsnFormat::Specctra);
        assert!(parse_dsn(d).is_err());
    }
    for d in [
        b"(pcbfoo board)".as_slice(),
        b"; only comment",
        b"ISIS SCHEMATIC FILE",
    ] {
        assert_eq!(detect_dsn_format(d), DsnFormat::Unknown);
    }
}
#[test]
fn omitted_circuit_sections_are_reported_explicitly() {
    let mut b = wire();
    b.extend(b"ISIS CIRCUIT FILE\x1aOBJECT DATA\0");
    let doc = parse_dsn(&b).unwrap();
    assert_eq!(doc.wires.len(), 1);
    let expected_warning = eda_i18n::format(
        "Seções de circuito adicionais não importadas: {omitted}. Apenas a primeira seção foi convertida.",
        &[("omitted", "1".to_owned())],
    );
    assert!(doc.warnings.contains(&expected_warning));
}
#[test]
fn modern_project_and_backup_report_discarded_pcb_and_members() {
    let root = tempfile::tempdir().unwrap();
    for ext in ["pdsprj", "pdsbak"] {
        let path = root.path().join(format!("test.{ext}"));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let data = wire();
        for (name, bytes) in [
            (
                "PROJECT.XML",
                b"<PROJECT><TIMESTAMP FILEVER='800' RELEASE='800'/></PROJECT>".as_slice(),
            ),
            ("ROOT.DSN", data.as_slice()),
            ("ROOT.LYT", b"pcb-layout"),
            ("firmware.hex", b":00000001FF"),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        let result = import_proteus(&path).unwrap();
        assert!(!result.report.electrical_complete);
        assert!(result.project.board.is_none());
        let expected_pcb_warning = eda_i18n::text(
            "PCB do contêiner não importada: posicionamentos, cobre, vias, furos, zonas e regras de fabricação permanecem somente no arquivo original.",
        );
        assert!(result.report.warnings.contains(&expected_pcb_warning));
        assert!(
            result
                .report
                .warnings
                .iter()
                .any(|w| w.contains("firmware.hex"))
        );
        let expected_format_warning = eda_i18n::text(
            "A conversão não preserva integralmente o formato Proteus e não permite reexportação fiel para DSN/PDSPrj. electrical_complete descreve apenas equivalência elétrica, não fidelidade de formato.",
        );
        assert!(result.report.warnings.contains(&expected_format_warning));
    }
}
#[test]
fn standalone_dsn_is_unversioned_and_lowercase_cdb_is_not_silently_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.dsn");
    std::fs::write(&path, wire()).unwrap();
    let result = import_proteus(&path).unwrap();
    assert_eq!(result.report.file_version, None);
    assert!(!result.report.electrical_complete);
    assert!(
        result
            .report
            .warnings
            .iter()
            .any(|w| w.contains("PROJECT.XML"))
    );
    std::fs::write(path.with_extension("cdb"), b"invalid CDB").unwrap();
    assert!(matches!(import_proteus(&path), Err(ImportError::Cdb(_))));
}
