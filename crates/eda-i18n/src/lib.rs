use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    English,
    PortugueseBr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    English,
    PortugueseBr,
}

static PREFERENCE: AtomicU8 = AtomicU8::new(0);
static SYSTEM_LANGUAGE: OnceLock<Language> = OnceLock::new();
static ENGLISH: OnceLock<HashMap<String, String>> = OnceLock::new();
static PORTUGUESE: OnceLock<HashMap<String, String>> = OnceLock::new();
static ENGLISH_EMBEDDED: OnceLock<HashMap<String, String>> = OnceLock::new();
static PORTUGUESE_EMBEDDED: OnceLock<HashMap<String, String>> = OnceLock::new();
static ENGLISH_SOURCES: OnceLock<HashMap<String, String>> = OnceLock::new();
static PORTUGUESE_SOURCES: OnceLock<HashMap<String, String>> = OnceLock::new();
type TemplateCandidate = (String, String, String);
static ENGLISH_TEMPLATES: OnceLock<Vec<TemplateCandidate>> = OnceLock::new();
static PORTUGUESE_TEMPLATES: OnceLock<Vec<TemplateCandidate>> = OnceLock::new();

pub fn set_preference(preference: LanguagePreference) {
    PREFERENCE.store(
        match preference {
            LanguagePreference::System => 0,
            LanguagePreference::English => 1,
            LanguagePreference::PortugueseBr => 2,
        },
        Ordering::Relaxed,
    );
}

pub fn preference_name(preference: LanguagePreference) -> &'static str {
    match preference {
        LanguagePreference::System => "language.system",
        LanguagePreference::English => "language.english",
        LanguagePreference::PortugueseBr => "language.portuguese_br",
    }
}

fn active_language() -> Language {
    match PREFERENCE.load(Ordering::Relaxed) {
        1 => Language::English,
        2 => Language::PortugueseBr,
        _ => *SYSTEM_LANGUAGE.get_or_init(|| {
            let locale = sys_locale::get_locale().unwrap_or_default();
            if locale
                .split(['-', '_'])
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case("pt"))
            {
                Language::PortugueseBr
            } else {
                Language::English
            }
        }),
    }
}

fn parse_catalog(source: &str) -> Result<HashMap<String, String>, serde_json::Error> {
    serde_json::from_str(source)
}

fn embedded_english() -> &'static HashMap<String, String> {
    ENGLISH_EMBEDDED.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/i18n/en.json")).unwrap_or_default()
    })
}

fn embedded_portuguese() -> &'static HashMap<String, String> {
    PORTUGUESE_EMBEDDED.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/i18n/pt-BR.json")).unwrap_or_default()
    })
}

fn report_catalog_problem(key: &str, values: &[(&str, String)]) {
    let (current, fallback) = match active_language() {
        Language::English => (embedded_english(), embedded_english()),
        Language::PortugueseBr => (embedded_portuguese(), embedded_english()),
    };
    let mut message = current
        .get(key)
        .or_else(|| fallback.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned());
    for (name, value) in values {
        message = message.replace(&format!("{{{name}}}"), value);
    }
    eprintln!("{message}");
}

fn override_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        directories.push(parent.join("i18n"));
    }

    if let Some(base) = std::env::var_os("APPDATA").map(PathBuf::from) {
        directories.push(base.join("ProtoCraft").join("i18n"));
    } else if let Some(base) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        directories.push(base.join("ProtoCraft").join("i18n"));
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        directories.push(home.join(".config").join("ProtoCraft").join("i18n"));
    }

    if let Some(directory) = std::env::var_os("PROTOCRAFT_I18N_DIR").map(PathBuf::from) {
        directories.push(directory);
    }
    directories
}

fn load_catalog(embedded: &str, file_name: &str) -> HashMap<String, String> {
    let mut catalog = match parse_catalog(embedded) {
        Ok(catalog) => catalog,
        Err(error) => {
            report_catalog_problem(
                "O catálogo {path} contém JSON inválido: {error}",
                &[
                    ("path", format!("{file_name} (incorporado)")),
                    ("error", error.to_string()),
                ],
            );
            HashMap::new()
        }
    };
    for directory in override_directories() {
        let path = directory.join(file_name);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                report_catalog_problem(
                    "Não foi possível ler o catálogo {path}: {error}",
                    &[
                        ("path", path.display().to_string()),
                        ("error", error.to_string()),
                    ],
                );
                continue;
            }
        };
        if bytes.len() > 2 * 1024 * 1024 {
            report_catalog_problem(
                "O catálogo {path} excede 2 MiB.",
                &[("path", path.display().to_string())],
            );
            continue;
        }
        let source = match std::str::from_utf8(&bytes) {
            Ok(source) => source,
            Err(error) => {
                report_catalog_problem(
                    "O catálogo {path} não está em UTF-8: {error}",
                    &[
                        ("path", path.display().to_string()),
                        ("error", error.to_string()),
                    ],
                );
                continue;
            }
        };
        match parse_catalog(source) {
            Ok(overlay) => catalog.extend(overlay),
            Err(error) => report_catalog_problem(
                "O catálogo {path} contém JSON inválido: {error}",
                &[
                    ("path", path.display().to_string()),
                    ("error", error.to_string()),
                ],
            ),
        }
    }
    catalog
}

fn english() -> &'static HashMap<String, String> {
    ENGLISH.get_or_init(|| load_catalog(include_str!("../../../assets/i18n/en.json"), "en.json"))
}

fn portuguese() -> &'static HashMap<String, String> {
    PORTUGUESE.get_or_init(|| {
        load_catalog(
            include_str!("../../../assets/i18n/pt-BR.json"),
            "pt-BR.json",
        )
    })
}

fn reverse_catalog(catalog: &HashMap<String, String>) -> HashMap<String, String> {
    let mut reverse = HashMap::with_capacity(catalog.len());
    for (key, value) in catalog {
        reverse.entry(value.clone()).or_insert_with(|| key.clone());
    }
    reverse
}

fn match_template(template: &str, input: &str) -> Option<Vec<(String, String)>> {
    let mut template_cursor = 0;
    let mut input_cursor = 0;
    let mut captures = Vec::new();

    while let Some(open_rel) = template[template_cursor..].find('{') {
        let open = template_cursor + open_rel;
        let close = template[open + 1..].find('}')? + open + 1;
        let name = &template[open + 1..close];
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return None;
        }

        let literal = &template[template_cursor..open];
        if !input[input_cursor..].starts_with(literal) {
            return None;
        }
        input_cursor += literal.len();

        let next_cursor = close + 1;
        if let Some(next_open_rel) = template[next_cursor..].find('{') {
            let next_open = next_cursor + next_open_rel;
            let separator = &template[next_cursor..next_open];
            if separator.is_empty() {
                return None;
            }
            let value_end = input[input_cursor..].find(separator)? + input_cursor;
            captures.push((name.to_owned(), input[input_cursor..value_end].to_owned()));
            input_cursor = value_end + separator.len();
            template_cursor = next_open;
        } else {
            let suffix = &template[next_cursor..];
            let value_end = if suffix.is_empty() {
                input.len()
            } else {
                input[input_cursor..].rfind(suffix)? + input_cursor
            };
            if !input[value_end..].ends_with(suffix) {
                return None;
            }
            captures.push((name.to_owned(), input[input_cursor..value_end].to_owned()));
            input_cursor = value_end + suffix.len();
            template_cursor = template.len();
        }
    }

    let suffix = &template[template_cursor..];
    if !input[input_cursor..].starts_with(suffix) || input_cursor + suffix.len() != input.len() {
        return None;
    }
    Some(captures)
}

fn translate_template(
    input: &str,
    current: &HashMap<String, String>,
    fallback: &HashMap<String, String>,
    candidates: &[TemplateCandidate],
) -> Option<String> {
    for (template, source, translated_fallback) in candidates {
        let Some(values) = match_template(template, input) else {
            continue;
        };
        let mut result = current
            .get(source)
            .or_else(|| fallback.get(source))
            .map_or_else(|| translated_fallback.clone(), Clone::clone);
        for (name, value) in values {
            result = result.replace(&format!("{{{name}}}"), &format_argument(&name, &value));
        }
        return Some(result);
    }
    None
}

fn english_sources() -> &'static HashMap<String, String> {
    ENGLISH_SOURCES.get_or_init(|| reverse_catalog(english()))
}

fn portuguese_sources() -> &'static HashMap<String, String> {
    PORTUGUESE_SOURCES.get_or_init(|| reverse_catalog(portuguese()))
}

fn build_template_candidates(
    current: &HashMap<String, String>,
    other: &HashMap<String, String>,
    fallback: &HashMap<String, String>,
) -> Vec<TemplateCandidate> {
    let mut candidates = Vec::new();
    for catalog in [current, other] {
        for (source, translated) in catalog {
            if !current.contains_key(source) && !fallback.contains_key(source) {
                continue;
            }
            if source.contains('{') {
                candidates.push((source.clone(), source.clone(), translated.clone()));
            }
            if translated.contains('{') {
                candidates.push((translated.clone(), source.clone(), translated.clone()));
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    candidates
}

fn template_candidates(language: Language) -> &'static [TemplateCandidate] {
    match language {
        Language::English => ENGLISH_TEMPLATES
            .get_or_init(|| build_template_candidates(english(), portuguese(), english())),
        Language::PortugueseBr => PORTUGUESE_TEMPLATES
            .get_or_init(|| build_template_candidates(portuguese(), english(), english())),
    }
}

/// Resolve uma chave, sempre recorrendo ao inglês quando faltar uma tradução.
pub fn text(key: &str) -> String {
    let language = active_language();
    let (current, fallback, reverse_current) = match language {
        Language::English => (english(), english(), english_sources()),
        Language::PortugueseBr => (portuguese(), english(), portuguese_sources()),
    };
    if let Some(value) = current.get(key).or_else(|| fallback.get(key)) {
        return value.clone();
    }
    if let Some(source_key) = reverse_current.get(key) {
        if let Some(value) = current.get(source_key).or_else(|| fallback.get(source_key)) {
            return value.clone();
        }
    }
    if let Some(value) = translate_template(key, current, fallback, template_candidates(language)) {
        return value;
    }
    key.to_owned()
}

/// Formata placeholders nomeados depois da tradução, permitindo reordená-los por idioma.
pub fn format(key: &str, values: &[(&str, String)]) -> String {
    let mut message = text(key);
    for (name, value) in values {
        message = message.replace(&format!("{{{name}}}"), &format_argument(name, value));
    }
    message
}

fn format_argument(name: &str, value: &str) -> String {
    if matches!(
        name,
        "error"
            | "preserve_error"
            | "warning"
            | "message"
            | "description"
            | "engine"
            | "status"
            | "severity"
            | "0"
    ) {
        text(value)
    } else {
        value.to_owned()
    }
}
