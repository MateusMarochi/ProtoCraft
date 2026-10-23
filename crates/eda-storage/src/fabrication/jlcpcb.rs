//! Preset de arquivos de fabricação para PCB de duas camadas; não altera regras da placa.
use eda_core::{NM_PER_MM, Project};
use std::{fs, io::Write, path::Path};

pub(super) const GERBER_OPTIONS: &[&str] = &[
    "--no-x2",
    "--no-netlist",
    "--precision",
    "6",
    "--subtract-soldermask",
];
pub(super) const DRILL_OPTIONS: &[&str] = &[
    "--format",
    "excellon",
    "--drill-origin",
    "absolute",
    "--excellon-units",
    "mm",
    "--excellon-zeros-format",
    "decimal",
    "--excellon-oval-format",
    "alternate",
    "--excellon-separate-th",
];
const GERBERS: &[&str] = &[
    "gtl", "gbl", "gts", "gbs", "gtp", "gbp", "gto", "gbo", "gm1",
];
const MAX_CAM_BYTES: u64 = 64 * 1024 * 1024;

fn read_cam(path: &Path) -> Result<String, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_CAM_BYTES {
        return Err("Arquivo de fabricação excede 64 MiB.".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

// O núcleo exporta somente contorno retangular. Confere fechamento e escala nas
// coordenadas reais do Gerber, antes de colocá-lo no ZIP para a fábrica.
fn check_outline(text: &str, project: &Project) -> Result<(), String> {
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let corners = [
        (0, 0),
        (board.width_nm, 0),
        (board.width_nm, -board.height_nm),
        (0, -board.height_nm),
        (0, 0),
    ];
    let mut vertices = Vec::new();
    for line in text.lines().map(str::trim) {
        if !line.starts_with('X') {
            continue;
        }
        let (coords, operation) = line.split_once('D').ok_or("Contorno Gerber inválido")?;
        let (x, y) = coords[1..]
            .split_once('Y')
            .ok_or("Contorno sem coordenadas XY")?;
        let point = (
            x.parse::<i64>().map_err(|_| "Coordenada X inválida")?,
            y.parse::<i64>().map_err(|_| "Coordenada Y inválida")?,
        );
        let expected = if vertices.is_empty() { "02*" } else { "01*" };
        if operation != expected {
            return Err("Contorno Gerber não é um caminho fechado único.".into());
        }
        vertices.push(point);
    }
    if vertices != corners {
        return Err("Contorno Gerber difere da PCB em escala 1:1/origem absoluta.".into());
    }
    Ok(())
}

pub(super) fn package(directory: &Path, project: &Project) -> Result<(), String> {
    let mut files: Vec<_> = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    files.sort();
    let mut contents = Vec::new();
    let mut extensions = std::collections::BTreeSet::new();
    for file in files {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !GERBERS.contains(&ext) && ext != "drl" {
            continue;
        }
        let text = read_cam(&file)?;
        if ext == "drl" {
            if !text.starts_with("M48")
                || !text.lines().any(|l| l.trim() == "METRIC")
                || !text.lines().any(|l| l.trim() == "G90")
                || !text.contains("absolute / metric / decimal")
                || !text.trim_end().ends_with("M30")
            {
                return Err(
                    "Excellon fora do preset JLCPCB: exige mm, origem absoluta e decimal.".into(),
                );
            }
        } else {
            if !extensions.insert(ext.to_owned()) {
                return Err(format!("Camada Gerber duplicada: {ext}"));
            }
            if !text.contains("%FSLAX46Y46*%")
                || !text.contains("%MOMM*%")
                || ["%TF", "%TA", "%TO"].iter().any(|v| text.contains(v))
                || !text.trim_end().ends_with("M02*")
            {
                return Err(format!(
                    "Gerber fora do preset RS-274X/mm/4.6: {file}",
                    file = file.display()
                ));
            }
            if ext == "gm1" {
                check_outline(&text, project)?;
            }
        }
        let name = file
            .with_extension(ext.to_ascii_uppercase())
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Nome de arquivo inválido")?
            .to_owned();
        contents.push((name, text));
    }
    if extensions.len() != GERBERS.len() || !contents.iter().any(|(n, _)| n.ends_with(".DRL")) {
        return Err("Preset JLCPCB exige nove camadas Protel e furação Excellon.".into());
    }
    let archive = fs::File::create(directory.join("JLCPCB.zip")).map_err(|e| e.to_string())?;
    let mut writer = zip::ZipWriter::new(archive);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in &contents {
        writer
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    writer
        .finish()
        .map_err(|e| e.to_string())?
        .sync_all()
        .map_err(|e| e.to_string())?;
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let manifest = serde_json::json!({
        "preset": "jlcpcb", "preset_version": 1, "archive": "JLCPCB.zip",
        "gerber": "RS-274X", "extensions": "Protel", "units": "mm",
        "coordinate_format": "4.6", "drill": "Excellon", "drill_origin": "absolute",
        "zeros": "decimal", "separate_pth_npth": true, "scale": "1:1", "mirror": false,
        "outline": "closed_rectangle_on_Edge.Cuts", "copper_layers": 2,
        "board_mm": { "width": board.width_nm as f64 / NM_PER_MM as f64,
            "height": board.height_nm as f64 / NM_PER_MM as f64 },
        "files": contents.iter().map(|(n, _)| n).collect::<Vec<_>>(),
        "reference": "https://jlcpcb.com/help/article/how-to-generate-gerber-and-drill-files-in-kicad-9"
    });
    fs::write(
        directory.join("preset-jlcpcb.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
