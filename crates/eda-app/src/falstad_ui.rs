use eda_import::ImportResult;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

pub struct ImportTask {
    pub receiver: mpsc::Receiver<Result<ImportResult, String>>,
    pub revision: u64,
    pub source: Option<PathBuf>,
}

pub fn is_source(input: &str) -> bool {
    input.starts_with("https://")
        || Path::new(input)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "txt" | "xml" | "url"))
}

pub fn start(
    input: String,
    revision: u64,
    ctx: &eframe::egui::Context,
) -> Result<ImportTask, String> {
    let source = if input.starts_with("https://") {
        None
    } else {
        Some(PathBuf::from(&input))
    };
    let (sender, receiver) = mpsc::channel();
    let repaint = ctx.clone();
    std::thread::Builder::new()
        .name("falstad-import".into())
        .spawn(move || {
            let result = eda_storage::falstad_source::read_source(&input).and_then(|text| {
                let name = source_name(&input);
                eda_import::falstad::import_text(&text, &name)
            });
            let _ = sender.send(result);
            repaint.request_repaint_after(Duration::from_millis(1));
        })
        .map_err(|error| error.to_string())?;
    Ok(ImportTask {
        receiver,
        revision,
        source,
    })
}

fn source_name(input: &str) -> String {
    if input.starts_with("https://") {
        return "Falstad".into();
    }
    Path::new(input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Falstad")
        .into()
}
