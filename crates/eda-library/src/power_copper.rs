//! Estimativas de cobre para planejamento; não substituem análise térmica da placa.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopperTraceSpec {
    pub current_a: f64,
    pub copper_thickness_um: f64,
    pub temperature_rise_c: f64,
    pub ambient_temperature_c: f64,
    pub length_mm: f64,
    pub maximum_voltage_drop_v: f64,
    pub width_margin: f64,
    pub internal_layer: bool,
}

impl Default for CopperTraceSpec {
    fn default() -> Self {
        Self {
            current_a: 3.0,
            copper_thickness_um: 70.0,
            temperature_rise_c: 10.0,
            ambient_temperature_c: 25.0,
            length_mm: 30.0,
            maximum_voltage_drop_v: 0.05,
            width_margin: 1.15,
            internal_layer: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopperTraceResult {
    pub thermal_minimum_width_mm: f64,
    pub voltage_drop_minimum_width_mm: f64,
    pub selected_width_mm: f64,
    pub resistance_estimate_ohms: f64,
    pub voltage_drop_estimate_v: f64,
    pub loss_estimate_w: f64,
}

impl CopperTraceSpec {
    pub fn calculate(self) -> Result<CopperTraceResult, String> {
        let values = [
            self.current_a,
            self.copper_thickness_um,
            self.temperature_rise_c,
            self.ambient_temperature_c,
            self.length_mm,
            self.maximum_voltage_drop_v,
            self.width_margin,
        ];
        if values.iter().any(|value| !value.is_finite())
            || !(0.05..=20.0).contains(&self.current_a)
            || !(9.0..=420.0).contains(&self.copper_thickness_um)
            || !(1.0..=100.0).contains(&self.temperature_rise_c)
            || !(-40.0..=85.0).contains(&self.ambient_temperature_c)
            || !(0.1..=2_000.0).contains(&self.length_mm)
            || !(0.001..=10.0).contains(&self.maximum_voltage_drop_v)
            || !(1.0..=2.0).contains(&self.width_margin)
        {
            return Err(eda_i18n::text(
                "Parâmetros de dimensionamento de cobre inválidos.",
            ));
        }
        // Ajuste histórico IPC-2221, também publicado na nota TI SNVA766.
        // O resultado é uma estimativa explícita; não representa conformidade IPC-2152.
        let k = if self.internal_layer { 0.024 } else { 0.048 };
        let area_square_mils =
            (self.current_a / (k * self.temperature_rise_c.powf(0.44))).powf(1.0 / 0.725);
        let thermal_width = area_square_mils / (self.copper_thickness_um / 25.4) * 0.0254;
        let resistivity = 1.724e-8
            * (1.0 + 0.00393 * (self.ambient_temperature_c + self.temperature_rise_c - 20.0));
        let thickness_m = self.copper_thickness_um * 1e-6;
        let length_m = self.length_mm * 1e-3;
        let voltage_width = resistivity * length_m * self.current_a
            / (thickness_m * self.maximum_voltage_drop_v)
            * 1e3;
        let selected_width =
            (thermal_width.max(voltage_width) * self.width_margin / 0.05).ceil() * 0.05;
        let resistance = resistivity * length_m / (thickness_m * selected_width * 1e-3);
        Ok(CopperTraceResult {
            thermal_minimum_width_mm: thermal_width,
            voltage_drop_minimum_width_mm: voltage_width,
            selected_width_mm: selected_width,
            resistance_estimate_ohms: resistance,
            voltage_drop_estimate_v: self.current_a * resistance,
            loss_estimate_w: self.current_a.powi(2) * resistance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_sizing_combines_thermal_and_voltage_drop_constraints() {
        let result = CopperTraceSpec::default()
            .calculate()
            .expect("especificação válida");
        assert!((result.thermal_minimum_width_mm - 0.683).abs() < 0.01);
        assert!(result.selected_width_mm >= 0.8);
        assert!(result.voltage_drop_estimate_v < 0.05);
        assert!((result.loss_estimate_w - result.voltage_drop_estimate_v * 3.0).abs() < 1e-12);
        let thin_copper = CopperTraceSpec {
            copper_thickness_um: 35.0,
            ..Default::default()
        }
        .calculate()
        .unwrap();
        assert!(thin_copper.selected_width_mm > result.selected_width_mm * 1.9);
        let long_trace = CopperTraceSpec {
            length_mm: 200.0,
            ..Default::default()
        }
        .calculate()
        .unwrap();
        assert!(long_trace.voltage_drop_minimum_width_mm > long_trace.thermal_minimum_width_mm);
        assert!(
            CopperTraceSpec {
                current_a: f64::NAN,
                ..Default::default()
            }
            .calculate()
            .is_err()
        );
    }
}
