use eda_storage::falstad_source::read_source;

const ORIGINAL: &str = "$ 1 0.000005 10.2 50 5 50\nr 0 0 32 0 0 1000\n";
// Texto original acima comprimido pela referência MIT lz-string 1.4.5 de Pieroxy.
const CTZ: &str = "CQAgjCAMB0l3BWcMBMIGXeyAoATlIQMxqaZjw5A";

#[test]
fn inline_links_are_offline_and_preserve_percent_and_utf16_text() {
    assert_eq!(
        read_source(&format!(
            "https://www.falstad.com/circuit/circuitjs.html?ctz={CTZ}"
        ))
        .unwrap(),
        ORIGINAL
    );
    assert_eq!(
        read_source("https://falstad.com/circuit/?cct=%24%201%0Ar%200%200%2032%200%200%201000%0A")
            .unwrap(),
        "$ 1\nr 0 0 32 0 0 1000\n"
    );
    assert_eq!(
        read_source("https://circuitjs1.falstad.com/?ctz=B4AkvBuGV7IOcFBA").unwrap(),
        "x 🚦 ç\n"
    );
    assert_eq!(
        read_source("https://falstad.com/?cct=x%2520y").unwrap(),
        "x%20y"
    );
}

#[test]
fn local_text_xml_and_internet_shortcuts_preserve_original_files() {
    let dir = tempfile::tempdir().unwrap();
    for extension in ["txt", "xml"] {
        let path = dir.path().join(format!("source.{extension}"));
        std::fs::write(&path, ORIGINAL).unwrap();
        assert_eq!(read_source(path.to_str().unwrap()).unwrap(), ORIGINAL);
        assert_eq!(std::fs::read_to_string(path).unwrap(), ORIGINAL);
    }
    let shortcut = dir.path().join("circuit.url");
    let original = format!("\u{feff}[InternetShortcut]\r\nURL=https://falstad.com/?ctz={CTZ}\r\n");
    std::fs::write(&shortcut, &original).unwrap();
    assert_eq!(read_source(shortcut.to_str().unwrap()).unwrap(), ORIGINAL);
    assert_eq!(std::fs::read_to_string(&shortcut).unwrap(), original);
    std::fs::write(
        &shortcut,
        "[InternetShortcut]\nURL=https://falstad.com/?cct=a\nURL=https://falstad.com/?cct=b",
    )
    .unwrap();
    assert!(read_source(shortcut.to_str().unwrap()).is_err());
}

#[test]
fn malformed_unsafe_and_unsupported_links_are_refused_without_fetch() {
    for bad in [
        "http://falstad.com/?cct=x",
        "https://evil.example/?cct=x",
        "https://falstad.com.evil.example/?cct=x",
        "https://user:pass@falstad.com/?cct=x",
        "https://@falstad.com/?cct=x",
        "https://falstad.com:444/?cct=x",
        "https://falstad.com/?cct=%zz",
        "https://falstad.com/?cct=%ff",
        "https://falstad.com/?ctz=",
        "https://falstad.com/?ctz=-------",
        "https://falstad.com/?cct=x&ctz=x",
        "https://falstad.com/?startCircuitLink=https%3A%2F%2Fexample.com%2Fc.txt",
        "https://falstad.com/?cct=x&startCircuitLink=x",
        "https://falstad.com/s.py?s=abc&s=def",
        "https://falstad.com/s.py?s=abc&unknown=1",
        "https://falstad.com/not-a-shortlink",
    ] {
        assert!(read_source(bad).is_err(), "{bad}");
    }
    assert!(
        read_source(&format!(
            "https://falstad.com/?ctz={}",
            &CTZ[..CTZ.len() / 2]
        ))
        .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.txt");
    std::fs::write(&path, vec![b'a'; 2 * 1024 * 1024 + 1]).unwrap();
    assert!(read_source(path.to_str().unwrap()).is_err());
}

// Emite o caso LZW KwKwK (A, AA, AAA...), com largura de código LZ-string.
fn expanding_link(entries: usize) -> String {
    fn emit(bits: &mut Vec<bool>, value: usize, width: usize) {
        for index in 0..width {
            bits.push(value & (1 << index) != 0);
        }
    }
    let mut bits = Vec::new();
    emit(&mut bits, 0, 2);
    emit(&mut bits, b'A' as usize, 8);
    let mut width = 3;
    let mut enlarge = 4;
    for code in 4..4 + entries {
        emit(&mut bits, code, width);
        enlarge -= 1;
        if enlarge == 0 {
            enlarge = 1 << width;
            width += 1;
        }
    }
    emit(&mut bits, 2, width);
    while bits.len() % 6 != 0 {
        bits.push(false);
    }
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+-";
    let encoded: String = bits
        .chunks(6)
        .map(|chunk| alphabet[chunk.iter().fold(0, |n, b| n * 2 + usize::from(*b))] as char)
        .collect();
    format!("https://falstad.com/?ctz={encoded}")
}
#[test]
fn compressed_expansion_is_bounded_before_large_allocation() {
    assert_eq!(read_source(&expanding_link(10)).unwrap(), "A".repeat(66));
    assert!(read_source(&expanding_link(1500)).is_err());
}
