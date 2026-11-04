//! Leitor limitado de S-expressions para os formatos abertos KiCad.

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 200_000;
const MAX_ATOM_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    pub fn atom(&self) -> Option<&str> {
        if let Self::Atom(value) = self {
            Some(value)
        } else {
            None
        }
    }

    pub fn list(&self) -> Option<&[SExpr]> {
        if let Self::List(values) = self {
            Some(values)
        } else {
            None
        }
    }

    pub fn head(&self) -> Option<&str> {
        self.value(0)
    }

    pub fn child(&self, name: &str) -> Option<&SExpr> {
        self.children(name).next()
    }

    pub fn children<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a SExpr> + 'a {
        let name = name.to_owned();
        self.list()
            .unwrap_or(&[])
            .iter()
            .filter(move |value| value.head() == Some(name.as_str()))
    }

    /// O índice inclui o átomo de cabeçalho na posição zero.
    pub fn value(&self, index: usize) -> Option<&str> {
        self.list()?.get(index)?.atom()
    }
}

fn error() -> String {
    eda_i18n::text("S-expression inválida ou acima dos limites de segurança.")
}

pub fn parse(input: &str) -> Result<SExpr, String> {
    if input.len() > MAX_BYTES {
        return Err(error());
    }

    struct Reader<'a> {
        source: &'a str,
        position: usize,
        nodes: usize,
    }
    impl Reader<'_> {
        fn skip_layout(&mut self) -> Result<(), String> {
            loop {
                while let Some(character) = self.source[self.position..].chars().next() {
                    if character.is_whitespace() {
                        self.position += character.len_utf8();
                    } else {
                        break;
                    }
                }
                if self.source.as_bytes().get(self.position) != Some(&b';') {
                    return Ok(());
                }
                let end = self.source[self.position..]
                    .find('\n')
                    .map_or(self.source.len(), |offset| self.position + offset + 1);
                self.position = end;
            }
        }

        fn item(&mut self, depth: usize) -> Result<SExpr, String> {
            self.skip_layout()?;
            self.nodes = self.nodes.checked_add(1).ok_or_else(error)?;
            if depth > MAX_DEPTH || self.nodes > MAX_NODES {
                return Err(error());
            }
            let character = self.source[self.position..]
                .chars()
                .next()
                .ok_or_else(error)?;
            match character {
                '(' => {
                    self.position += 1;
                    let mut values = Vec::new();
                    loop {
                        self.skip_layout()?;
                        if self.source.as_bytes().get(self.position) == Some(&b')') {
                            self.position += 1;
                            return Ok(SExpr::List(values));
                        }
                        if self.position >= self.source.len() {
                            return Err(error());
                        }
                        values.push(self.item(depth + 1)?);
                    }
                }
                ')' => Err(error()),
                '"' => self.quoted(),
                _ => self.bare(),
            }
        }

        fn quoted(&mut self) -> Result<SExpr, String> {
            self.position += 1;
            let mut output = String::new();
            loop {
                let character = self.source[self.position..]
                    .chars()
                    .next()
                    .ok_or_else(error)?;
                self.position += character.len_utf8();
                match character {
                    '"' => return Ok(SExpr::Atom(output)),
                    '\\' => {
                        let escaped = self.source[self.position..]
                            .chars()
                            .next()
                            .ok_or_else(error)?;
                        self.position += escaped.len_utf8();
                        match escaped {
                            '"' => output.push('"'),
                            '\\' => output.push('\\'),
                            'n' => output.push('\n'),
                            'r' => output.push('\r'),
                            't' => output.push('\t'),
                            _ => return Err(error()),
                        }
                    }
                    value if value.is_control() => return Err(error()),
                    value => output.push(value),
                }
                if output.len() > MAX_ATOM_BYTES {
                    return Err(error());
                }
            }
        }

        fn bare(&mut self) -> Result<SExpr, String> {
            let start = self.position;
            while let Some(character) = self.source[self.position..].chars().next() {
                if character.is_whitespace() || matches!(character, '(' | ')' | ';') {
                    break;
                }
                if character == '"' || character.is_control() {
                    return Err(error());
                }
                self.position += character.len_utf8();
            }
            if self.position == start || self.position - start > MAX_ATOM_BYTES {
                return Err(error());
            }
            let value = &self.source[start..self.position];
            if matches!(
                value.to_ascii_lowercase().as_str(),
                "nan" | "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity"
            ) {
                return Err(error());
            }
            Ok(SExpr::Atom(value.to_owned()))
        }
    }

    let source = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut reader = Reader {
        source,
        position: 0,
        nodes: 0,
    };
    let expression = reader.item(0)?;
    reader.skip_layout()?;
    if reader.position != source.len() {
        return Err(error());
    }
    Ok(expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bom_comments_and_escaped_strings() {
        let parsed =
            parse("\u{feff}; cabeçalho\n(root (name \"R1 \\\"alta\\\"\\nlinha\") ; fim\n)")
                .expect("S-expression válida");
        assert_eq!(parsed.head(), Some("root"));
        assert_eq!(
            parsed.child("name").and_then(|node| node.value(1)),
            Some("R1 \"alta\"\nlinha")
        );
    }

    #[test]
    fn rejects_trailing_expressions_and_unterminated_strings() {
        assert!(parse("(root) (extra)").is_err());
        assert!(parse("(root \"broken)").is_err());
    }

    #[test]
    fn enforces_depth_node_and_document_limits() {
        let too_deep = format!(
            "{}x{}",
            "(".repeat(MAX_DEPTH + 2),
            ")".repeat(MAX_DEPTH + 2)
        );
        assert!(parse(&too_deep).is_err());
        assert!(parse(&format!("(root {})", "x ".repeat(MAX_NODES))).is_err());
        assert!(parse(&" ".repeat(MAX_BYTES + 1)).is_err());
    }
}
