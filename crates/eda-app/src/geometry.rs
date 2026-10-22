use eda_core::Point;

/// Distância em unidades da grade, usada também para selecionar fios sem GPU.
pub fn distance_to_segment(p: [f32; 2], a: Point, b: Point) -> f32 {
    let a = [a.x as f32, a.y as f32];
    let b = [b.x as f32, b.y as f32];
    let delta = [b[0] - a[0], b[1] - a[1]];
    let length = delta[0] * delta[0] + delta[1] * delta[1];
    let t = if length > 0.0 {
        (((p[0] - a[0]) * delta[0] + (p[1] - a[1]) * delta[1]) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * delta[0]).powi(2) + (p[1] - a[1] - t * delta[1]).powi(2)).sqrt()
}

pub fn orthogonal_corner(start: Point, end: Point, vertical_first: bool) -> Point {
    if vertical_first {
        Point {
            x: start.x,
            y: end.y,
        }
    } else {
        Point {
            x: end.x,
            y: start.y,
        }
    }
}

/// Entrada humana SI; M é mega, m é mili, e "meg" também é aceito.
pub fn parse_value(text: &str) -> Result<f64, String> {
    let text = text.trim().replace(',', ".").replace(['µ', 'μ'], "u");
    let units = [
        ("meg", 1e6),
        ("G", 1e9),
        ("M", 1e6),
        ("k", 1e3),
        ("m", 1e-3),
        ("u", 1e-6),
        ("n", 1e-9),
        ("p", 1e-12),
    ];
    let (number, factor) = units
        .iter()
        .find_map(|(suffix, factor)| text.strip_suffix(suffix).map(|number| (number, *factor)))
        .unwrap_or((&text, 1.0));
    let value = number
        .trim()
        .parse::<f64>()
        .map_err(|_| "Valor inválido. Exemplos: 1000, 1k, 1u ou 1e-6.".to_owned())?
        * factor;
    if !value.is_finite() {
        return Err("O valor precisa ser finito.".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_segment_and_endpoints() {
        let a = Point { x: 0, y: 0 };
        let b = Point { x: 10, y: 0 };
        assert_eq!(distance_to_segment([5.0, 2.0], a, b), 2.0);
        assert_eq!(distance_to_segment([13.0, 4.0], a, b), 5.0);
        assert_eq!(distance_to_segment([0.0, 0.0], a, a), 0.0);
    }
    #[test]
    fn routes_two_orthogonal_legs() {
        let start = Point { x: 2, y: 3 };
        let end = Point { x: 7, y: 9 };
        assert_eq!(orthogonal_corner(start, end, false), Point { x: 7, y: 3 });
        assert_eq!(orthogonal_corner(start, end, true), Point { x: 2, y: 9 });
    }
    #[test]
    fn values_are_si_and_finite() {
        assert_eq!(parse_value("1k"), Ok(1000.0));
        assert!((parse_value("2,2u").unwrap() - 2.2e-6).abs() < 1e-18);
        assert_eq!(parse_value("1M"), Ok(1e6));
        assert_eq!(parse_value("1m"), Ok(0.001));
        assert!(parse_value("NaN").is_err());
        assert!(parse_value("infinito").is_err());
    }
}
