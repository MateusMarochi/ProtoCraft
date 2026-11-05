//! Leitura limitada de fontes Falstad, sem executar conteúdo dos circuitos.
//! O decodificador URI LZ segue o formato MIT de Pieroxy (lz-string, 2013),
//! com limites próprios de dicionário, expansão e trabalho; veja NOTICE.md.
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use url::Url;

const MAX_TEXT: usize = 2 * 1024 * 1024;
const MAX_RESPONSE: usize = 8 * 1024 * 1024;
fn error(key: &str) -> String {
    eda_i18n::text(key)
}
fn invalid() -> String {
    error("falstad.source.invalid")
}
fn limit() -> String {
    error("falstad.source.limit")
}

/// Lê texto local ou um link HTTPS Falstad autorizado. Somente shortlinks usam rede.
pub fn read_source(input: &str) -> Result<String, String> {
    if input.len() > MAX_RESPONSE {
        return Err(limit());
    }
    let trimmed = input.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return resolve(trimmed);
    }
    let path = Path::new(input);
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !["txt", "xml", "url"].contains(&extension.as_str()) {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| error("falstad.source.file"))?
        .take(MAX_TEXT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("falstad.source.file"))?;
    if bytes.len() > MAX_TEXT {
        return Err(limit());
    }
    let text = String::from_utf8(bytes).map_err(|_| invalid())?;
    let text = text.trim_start_matches('\u{feff}');
    if extension == "url" {
        let mut section = false;
        let mut link = None;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                section = line.eq_ignore_ascii_case("[InternetShortcut]");
            } else if section
                && let Some((key, value)) = line.split_once('=')
                && key.trim().eq_ignore_ascii_case("URL")
                && link.replace(value.trim()).is_some()
            {
                return Err(invalid());
            }
        }
        return resolve(link.ok_or_else(invalid)?);
    }
    if text.trim().starts_with("https://") || text.trim().starts_with("http://") {
        resolve(text.trim())
    } else {
        Ok(text.to_owned())
    }
}

fn authorized(input: &str) -> Result<Url, String> {
    if input.len() > MAX_RESPONSE || input.chars().any(char::is_control) {
        return Err(invalid());
    }
    if input.split_once("://").is_some_and(|(_, rest)| {
        rest.split(['/', '?', '#'])
            .next()
            .is_some_and(|authority| authority.contains('@'))
    }) {
        return Err(error("falstad.source.unsafe_url"));
    }
    let url = Url::parse(input).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !matches!(
            url.host_str(),
            Some("www.falstad.com" | "falstad.com" | "circuitjs1.falstad.com")
        )
    {
        return Err(error("falstad.source.unsafe_url"));
    }
    Ok(url)
}

fn inline(url: &Url) -> Result<Option<String>, String> {
    let mut value = None;
    for pair in url
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|s| !s.is_empty())
    {
        let (key, raw) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode_query(key)?;
        let raw = decode_query(raw)?;
        if key == "startCircuitLink" {
            return Err(error("falstad.source.unsafe_url"));
        }
        if key == "cct" || key == "ctz" {
            if value.is_some() {
                return Err(invalid());
            }
            let text = if key == "ctz" {
                decompress(&raw.replace(' ', "+"))?
            } else {
                raw
            };
            if text.len() > MAX_TEXT {
                return Err(limit());
            }
            value = Some(text);
        }
    }
    Ok(value)
}
fn decode_query(input: &str) -> Result<String, String> {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(byte) = bytes.next() {
        output.push(match byte {
            b'%' => {
                let a = bytes
                    .next()
                    .and_then(|v| (v as char).to_digit(16))
                    .ok_or_else(invalid)?;
                let b = bytes
                    .next()
                    .and_then(|v| (v as char).to_digit(16))
                    .ok_or_else(invalid)?;
                ((a << 4) | b) as u8
            }
            b'+' => b' ',
            other => other,
        });
        if output.len() > MAX_TEXT {
            return Err(limit());
        }
    }
    String::from_utf8(output).map_err(|_| invalid())
}
fn resolve(input: &str) -> Result<String, String> {
    let mut url = authorized(input)?;
    let started = Instant::now();
    for redirect in 0..=5 {
        if let Some(text) = inline(&url)? {
            return Ok(text);
        }
        if url.path() != "/s.py"
            || url.fragment().is_some()
            || url.query_pairs().count() != 1
            || url.query_pairs().filter(|(k, _)| k == "s").count() != 1
        {
            return Err(invalid());
        }
        let token = url
            .query_pairs()
            .find(|(k, _)| k == "s")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(invalid)?;
        if token.is_empty()
            || token.len() > 128
            || !token
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(invalid());
        }
        if redirect == 5 {
            return Err(error("falstad.source.redirect_limit"));
        }
        let (status, location, body) = fetch(&url, started)?;
        if (300..400).contains(&status) {
            url = redirect_target(&url, location.as_deref().ok_or_else(invalid)?)?;
        } else if status == 200 {
            let text = String::from_utf8(body).map_err(|_| invalid())?;
            // Alguns serviços retornam o link puro; nunca interpretar HTML/JavaScript.
            if text.trim().starts_with("https://") {
                url = authorized(text.trim())?;
            } else {
                return Err(invalid());
            }
        } else {
            return Err(error("falstad.source.http"));
        }
    }
    Err(error("falstad.source.redirect_limit"))
}
fn redirect_target(current: &Url, location: &str) -> Result<Url, String> {
    if location.chars().any(char::is_control) {
        return Err(invalid());
    }
    let target = current.join(location).map_err(|_| invalid())?;
    // Validar também a grafia original para recusar userinfo vazio normalizado pelo URL.
    if location.contains("://") {
        authorized(location)?;
    }
    authorized(target.as_str())
}

fn fetch(url: &Url, started: Instant) -> Result<(u16, Option<String>, Vec<u8>), String> {
    let remaining = Duration::from_secs(15).saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(error("falstad.source.timeout"));
    }
    let headers = tempfile::NamedTempFile::new().map_err(|_| error("falstad.source.file"))?;
    let executable = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut child = Command::new(executable)
        .args([
            "--disable",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--max-time",
            &remaining.as_secs_f64().to_string(),
            "--connect-timeout",
            "5",
            "--max-filesize",
            &MAX_RESPONSE.to_string(),
            "--suppress-connect-headers",
            "--dump-header",
        ])
        .arg(headers.path())
        .args(["--output", "-", "--url", url.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| error("falstad.source.curl"))?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error("falstad.source.curl"));
        }
    };
    let (send, receive) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(MAX_RESPONSE as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = send.send(result);
    });
    let mut body = None;
    let result = loop {
        if started.elapsed() >= Duration::from_secs(15) {
            break Err(error("falstad.source.timeout"));
        }
        match headers.as_file().metadata() {
            Ok(m) if m.len() > 65_536 => break Err(limit()),
            Err(_) => break Err(invalid()),
            _ => {}
        }
        match receive.try_recv() {
            Ok(Ok(bytes)) if bytes.len() > MAX_RESPONSE => break Err(limit()),
            Ok(Ok(bytes)) => body = Some(bytes),
            Ok(Err(_)) => break Err(error("falstad.source.curl")),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {}
        }
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => break Err(error("falstad.source.http")),
            Ok(Some(_)) => {
                if let Some(body) = body.take() {
                    break Ok(body);
                }
                match receive.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(bytes)) if bytes.len() <= MAX_RESPONSE => break Ok(bytes),
                    Ok(Ok(_)) => break Err(limit()),
                    Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break Err(error("falstad.source.curl"));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            Ok(None) => {}
            Err(_) => break Err(error("falstad.source.curl")),
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Sempre recolher o processo antes de aguardar o leitor, inclusive em erro.
    if result.is_err() {
        let _ = child.kill();
    }
    let waited = child.wait();
    let joined = reader.join();
    if waited.is_err() || joined.is_err() {
        return Err(error("falstad.source.curl"));
    }
    let body = result?;
    let mut header_bytes = Vec::new();
    headers
        .as_file()
        .take(65_537)
        .read_to_end(&mut header_bytes)
        .map_err(|_| invalid())?;
    if header_bytes.len() > 65_536 {
        return Err(limit());
    }
    let header = String::from_utf8(header_bytes).map_err(|_| invalid())?;
    parse_response_headers(&header).map(|(status, location)| (status, location, body))
}
fn parse_response_headers(header: &str) -> Result<(u16, Option<String>), String> {
    let mut status = None;
    let mut location = None;
    for line in header.lines() {
        if line.starts_with("HTTP/") {
            status = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u16>().ok());
            location = None;
        } else if let Some((key, value)) = line.split_once(':')
            && key.eq_ignore_ascii_case("location")
            && location.replace(value.trim().to_owned()).is_some()
        {
            return Err(invalid());
        }
    }
    Ok((status.ok_or_else(invalid)?, location))
}

struct Bits {
    values: Vec<u8>,
    offset: usize,
}
impl Bits {
    fn read(&mut self, count: usize) -> Result<usize, String> {
        if count > 20
            || self
                .offset
                .checked_add(count)
                .is_none_or(|end| end > self.values.len() * 6)
        {
            return Err(invalid());
        }
        let mut result = 0;
        for bit in 0..count {
            let position = self.offset;
            self.offset += 1;
            let value = (self.values[position / 6] >> (5 - position % 6)) & 1;
            result |= usize::from(value) << bit;
        }
        Ok(result)
    }
}
fn decompress(input: &str) -> Result<String, String> {
    if input.is_empty() || input.len() > MAX_RESPONSE {
        return Err(invalid());
    }
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+-";
    let values = input
        .bytes()
        .map(|c| {
            alphabet
                .iter()
                .position(|v| *v == c)
                .map(|v| v as u8)
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut bits = Bits { values, offset: 0 };
    let first = match bits.read(2)? {
        0 => bits.read(8)? as u16,
        1 => bits.read(16)? as u16,
        2 => return Ok(String::new()),
        _ => return Err(invalid()),
    };
    let mut dictionary = vec![Vec::new(), Vec::new(), Vec::new(), vec![first]];
    let mut previous = vec![first];
    let mut output = vec![first];
    let mut width = 3;
    let mut enlarge = 4usize;
    let mut dictionary_units = 1usize;
    loop {
        if dictionary.len() >= 65_536
            || dictionary_units > MAX_RESPONSE
            || output.len() > MAX_TEXT / 2
        {
            return Err(limit());
        }
        let mut code = bits.read(width)?;
        if code == 2 {
            let text = String::from_utf16(&output).map_err(|_| invalid())?;
            return if text.len() <= MAX_TEXT {
                Ok(text)
            } else {
                Err(limit())
            };
        }
        if code < 2 {
            let literal = bits.read(if code == 0 { 8 } else { 16 })? as u16;
            code = dictionary.len();
            dictionary.push(vec![literal]);
            dictionary_units += 1;
            enlarge -= 1;
        }
        if enlarge == 0 {
            enlarge = 1usize << width;
            width += 1;
        }
        let entry = if code < dictionary.len() {
            dictionary[code].clone()
        } else if code == dictionary.len() {
            let mut v = previous.clone();
            v.push(previous[0]);
            v
        } else {
            return Err(invalid());
        };
        if entry.is_empty() {
            return Err(invalid());
        }
        if output.len() + entry.len() > MAX_TEXT / 2
            || dictionary_units + previous.len() + 1 > MAX_RESPONSE
        {
            return Err(limit());
        }
        output.extend_from_slice(&entry);
        let mut added = previous;
        added.push(entry[0]);
        dictionary_units += added.len();
        dictionary.push(added);
        previous = entry;
        enlarge -= 1;
        if enlarge == 0 {
            enlarge = 1usize << width;
            width += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_revalidate_hosts_scheme_credentials_and_port() {
        let origin = authorized("https://www.falstad.com/s.py?s=abc").unwrap();
        for bad in [
            "https://example.com/?cct=x",
            "http://falstad.com/?cct=x",
            "https://@falstad.com/?cct=x",
            "https://a@falstad.com/?cct=x",
            "https://falstad.com:444/?cct=x",
            "//example.com/?cct=x",
        ] {
            assert!(redirect_target(&origin, bad).is_err(), "{bad}");
        }
        assert_eq!(
            redirect_target(&origin, "/circuit/circuitjs.html?cct=x")
                .unwrap()
                .host_str(),
            Some("www.falstad.com")
        );
        assert!(
            parse_response_headers("HTTP/2 301\r\nLocation: /a\r\nLocation: /b\r\n\r\n").is_err()
        );
    }
}
