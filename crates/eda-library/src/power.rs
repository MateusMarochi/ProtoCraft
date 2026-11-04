//! Cálculos e fixtures de conversão DC/DC para projetos reutilizáveis.
use crate::catalog;
use eda_core::{
    Board, BoardSide, Component, ComponentId, ComponentKind, DataSource, DeviceDefinition,
    DevicePin, Footprint, FootprintLine, LabelId, MountingHole, NetClass, NmBounds, NmPoint, Pad,
    PadShape, PadTechnology, PinRef, Placement, Point, Project, Track,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const TI_DATASHEET: &str = "https://www.ti.com/lit/ds/symlink/lm61460.pdf";

/// Parâmetros de primeira ordem de um conversor buck idealizado.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuckDesignSpec {
    pub input_voltage_v: f64,
    pub output_voltage_v: f64,
    pub output_current_a: f64,
    pub regulator_current_limit_a: f64,
    pub switching_frequency_hz: f64,
    pub ripple_fraction_of_regulator_limit: f64,
    pub efficiency_assumption: f64,
    pub feedback_reference_v: f64,
    pub feedback_top_ohms: f64,
    pub selected_inductance_h: f64,
}

impl Default for BuckDesignSpec {
    fn default() -> Self {
        Self {
            input_voltage_v: 12.0,
            output_voltage_v: 5.1,
            output_current_a: 3.0,
            regulator_current_limit_a: 6.0,
            switching_frequency_hz: 400_000.0,
            ripple_fraction_of_regulator_limit: 0.25,
            efficiency_assumption: 0.92,
            feedback_reference_v: 1.0,
            feedback_top_ohms: 100_000.0,
            selected_inductance_h: 4.7e-6,
        }
    }
}

/// Resultados calculados sem executar um solver nem simular o componente real.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuckDesignResult {
    pub duty_cycle_ideal: f64,
    pub output_power_w: f64,
    pub input_current_estimate_a: f64,
    pub loss_estimate_w: f64,
    pub target_inductor_ripple_a: f64,
    pub minimum_inductance_h: f64,
    pub selected_inductor_ripple_a: f64,
    pub inductor_peak_current_at_load_a: f64,
    pub feedback_bottom_ideal_ohms: f64,
    pub feedback_bottom_selected_ohms: f64,
    pub output_voltage_nominal_v: f64,
}

impl BuckDesignSpec {
    pub fn validate(self) -> Result<(), String> {
        let values = [
            self.input_voltage_v,
            self.output_voltage_v,
            self.output_current_a,
            self.regulator_current_limit_a,
            self.switching_frequency_hz,
            self.ripple_fraction_of_regulator_limit,
            self.efficiency_assumption,
            self.feedback_reference_v,
            self.feedback_top_ohms,
            self.selected_inductance_h,
        ];
        if values.iter().any(|value| !value.is_finite())
            || !(3.0..=36.0).contains(&self.input_voltage_v)
            || self.input_voltage_v <= self.output_voltage_v
            || self.output_voltage_v > 0.95 * self.input_voltage_v
            || self.output_voltage_v <= self.feedback_reference_v
            || self.output_current_a <= 0.0
            || self.regulator_current_limit_a < self.output_current_a
            || self.regulator_current_limit_a > 6.0
            || !(200_000.0..=2_200_000.0).contains(&self.switching_frequency_hz)
            || !(0.05..=0.6).contains(&self.ripple_fraction_of_regulator_limit)
            || !(0.5..=1.0).contains(&self.efficiency_assumption)
            || self.feedback_reference_v <= 0.0
            || self.feedback_top_ohms <= 0.0
            || self.selected_inductance_h <= 0.0
        {
            return Err(eda_i18n::text(
                "Parâmetros do conversor buck inválidos ou fora do domínio suportado.",
            ));
        }
        Ok(())
    }

    pub fn calculate(self) -> Result<BuckDesignResult, String> {
        self.validate()?;
        let duty = self.output_voltage_v / self.input_voltage_v;
        let output_power = self.output_voltage_v * self.output_current_a;
        let input_current = output_power / (self.input_voltage_v * self.efficiency_assumption);
        let loss = output_power / self.efficiency_assumption - output_power;
        // A recomendação TI usa a corrente máxima do conversor quando a carga
        // é menor que sua capacidade, evitando subdimensionar o indutor.
        let target_ripple =
            self.regulator_current_limit_a * self.ripple_fraction_of_regulator_limit;
        let minimum_inductance = (self.input_voltage_v - self.output_voltage_v) * duty
            / (self.switching_frequency_hz * target_ripple);
        let selected_ripple = (self.input_voltage_v - self.output_voltage_v) * duty
            / (self.switching_frequency_hz * self.selected_inductance_h);
        let bottom_ideal = self.feedback_top_ohms * self.feedback_reference_v
            / (self.output_voltage_v - self.feedback_reference_v);
        let bottom_selected = nearest_e96(bottom_ideal);
        let output_nominal =
            self.feedback_reference_v * (1.0 + self.feedback_top_ohms / bottom_selected);
        Ok(BuckDesignResult {
            duty_cycle_ideal: duty,
            output_power_w: output_power,
            input_current_estimate_a: input_current,
            loss_estimate_w: loss,
            target_inductor_ripple_a: target_ripple,
            minimum_inductance_h: minimum_inductance,
            selected_inductor_ripple_a: selected_ripple,
            inductor_peak_current_at_load_a: self.output_current_a + selected_ripple / 2.0,
            feedback_bottom_ideal_ohms: bottom_ideal,
            feedback_bottom_selected_ohms: bottom_selected,
            output_voltage_nominal_v: output_nominal,
        })
    }
}

fn nearest_e96(value: f64) -> f64 {
    const E96: [f64; 96] = [
        100.0, 102.0, 105.0, 107.0, 110.0, 113.0, 115.0, 118.0, 121.0, 124.0, 127.0, 130.0, 133.0,
        137.0, 140.0, 143.0, 147.0, 150.0, 154.0, 158.0, 162.0, 165.0, 169.0, 174.0, 178.0, 182.0,
        187.0, 191.0, 196.0, 200.0, 205.0, 210.0, 215.0, 221.0, 226.0, 232.0, 237.0, 243.0, 249.0,
        255.0, 261.0, 267.0, 274.0, 280.0, 287.0, 294.0, 301.0, 309.0, 316.0, 324.0, 332.0, 340.0,
        348.0, 357.0, 365.0, 374.0, 383.0, 392.0, 402.0, 412.0, 422.0, 432.0, 442.0, 453.0, 464.0,
        475.0, 487.0, 499.0, 511.0, 523.0, 536.0, 549.0, 562.0, 576.0, 590.0, 604.0, 619.0, 634.0,
        649.0, 665.0, 681.0, 698.0, 715.0, 732.0, 750.0, 768.0, 787.0, 806.0, 825.0, 845.0, 866.0,
        887.0, 909.0, 931.0, 953.0, 976.0,
    ];
    let exponent = value.log10().floor() as i32;
    let decade = 10_f64.powi(exponent);
    E96.iter()
        .map(|value| value / 100.0 * decade)
        .min_by(|left, right| (left - value).abs().total_cmp(&(right - value).abs()))
        .unwrap_or(value)
}

/// Símbolo e footprint próprios do encapsulamento VQFN-HR do LM61460.
pub fn lm61460_definition() -> (DeviceDefinition, Footprint) {
    let source = DataSource {
        project: "Texas Instruments LM61460 datasheet".into(),
        revision: "SNVSBD5D, revisão D".into(),
        url: TI_DATASHEET.into(),
        license: "Dados factuais de pinos e dimensões do fabricante; geometria original ProtoCraft"
            .into(),
    };
    let pin_data = [
        ("BIAS", "power_in", Point::new(-8, -4)),
        ("VCC", "power_in", Point::new(-8, -2)),
        ("AGND", "passive", Point::new(-8, 0)),
        ("FB", "input", Point::new(-8, 2)),
        ("PGOOD", "open_collector", Point::new(-8, 4)),
        ("RT", "passive", Point::new(-2, 7)),
        ("EN/SYNC", "input", Point::new(0, 7)),
        ("VIN1", "power_in", Point::new(2, 7)),
        ("PGND1", "passive", Point::new(8, 2)),
        ("SW", "passive", Point::new(8, 0)),
        ("PGND2", "passive", Point::new(8, -2)),
        ("VIN2", "power_in", Point::new(2, -7)),
        ("RBOOT", "passive", Point::new(0, -7)),
        ("CBOOT", "passive", Point::new(-2, -7)),
    ];
    let pins = pin_data
        .iter()
        .enumerate()
        .map(|(index, (name, electrical_type, offset))| DevicePin {
            number: (index + 1).to_string(),
            name: (*name).into(),
            electrical_type: (*electrical_type).into(),
            offset: *offset,
            pads: vec![(index + 1).to_string()],
        })
        .collect();
    let device = DeviceDefinition {
        id: "LM61460".into(),
        requested_part: "LM61460AANRJRR".into(),
        manufacturer: "Texas Instruments".into(),
        family: "LM61460".into(),
        form: eda_i18n::text("VQFN-HR de 14 pinos, 4,0 × 3,5 mm"),
        description: eda_i18n::text(
            "Conversor síncrono buck de 3–36 V e até 6 A; validar dissipação, compensação e layout na aplicação.",
        ),
        datasheet: TI_DATASHEET.into(),
        pins,
        source: source.clone(),
    };
    let pad_positions = [
        (-1_650_000, -1_000_000, 700_000, 400_000),
        (-1_650_000, -500_000, 700_000, 400_000),
        (-1_650_000, 0, 700_000, 400_000),
        (-1_650_000, 500_000, 700_000, 400_000),
        (-1_650_000, 1_000_000, 700_000, 400_000),
        (-500_000, 1_450_000, 400_000, 700_000),
        (0, 1_450_000, 400_000, 700_000),
        (500_000, 1_450_000, 400_000, 700_000),
        (1_650_000, 1_000_000, 700_000, 400_000),
        (1_650_000, 0, 700_000, 400_000),
        (1_650_000, -1_000_000, 700_000, 400_000),
        (500_000, -1_450_000, 400_000, 700_000),
        (0, -1_450_000, 400_000, 700_000),
        (-500_000, -1_450_000, 400_000, 700_000),
    ];
    let pads = pad_positions
        .iter()
        .enumerate()
        .map(|(index, (x, y, width, height))| Pad {
            number: (index + 1).to_string(),
            position: NmPoint::new(*x, *y),
            size: NmPoint::new(*width, *height),
            rotation_mdeg: 0,
            shape: PadShape::Rect,
            technology: PadTechnology::Smd,
            drill: None,
            layers: vec!["F.Cu".into(), "F.Mask".into(), "F.Paste".into()],
            roundrect_ratio_ppm: 0,
            mask_margin_nm: 0,
            paste_margin_nm: 0,
            paste_ratio_ppm: 0,
            polygons: vec![],
        })
        .collect();
    let footprint = Footprint {
        id: "ProtoCraft_Power:VQFN-HR_4x3.5mm_P0.5mm".into(),
        description: eda_i18n::text(
            "LM61460 RJR0014A; land pattern preliminar do ProtoCraft, revisar máscara, pasta e stencil antes de fabricar.",
        ),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-2_000_000, -1_750_000),
            max: NmPoint::new(2_000_000, 1_750_000),
        },
        pads,
        lines: rectangle_lines(
            -2_000_000, -1_750_000, 2_000_000, 1_750_000, "F.Fab", 100_000,
        ),
        keepouts: vec![],
        source,
    };
    (device, footprint)
}

/// Fixture de potência 12 V → 5,1 V para Raspberry Pi 4, em perfil mecânico HAT.
pub fn buck_pi4_project() -> Result<Project, String> {
    let spec = BuckDesignSpec::default();
    spec.calculate()?;
    let catalog = catalog()?;
    let (gpio_device, gpio_footprint) = catalog.definition("RaspberryPi4B_GPIO")?;
    let (regulator, regulator_footprint) = lm61460_definition();
    let source = DataSource {
        project: "ProtoCraft original power design".into(),
        revision: "12 V to 5.1 V, 3 A prototype".into(),
        url: TI_DATASHEET.into(),
        license:
            "Geometria e circuito originais; valores de referência consultados no datasheet público"
                .into(),
    };
    let mut project = Project::empty(eda_i18n::text(
        "Raspberry Pi 4 — conversor buck 12 V para 5,1 V / 3 A",
    ));
    add_device(
        &mut project,
        ComponentId(1),
        "J_PI",
        gpio_device,
        gpio_footprint,
        Point::new(0, 0),
        0.0,
    );
    add_device(
        &mut project,
        ComponentId(2),
        "U1",
        regulator,
        regulator_footprint,
        Point::new(44, 28),
        0.0,
    );
    add_two_pin_device(
        &mut project,
        ComponentId(3),
        "J1",
        &eda_i18n::text("Entrada 12 V DC, positivo no pino 1"),
        "ProtoCraft_THT:Terminal_2P_5.08mm",
        12.0,
        two_pin_through_hole(
            &source,
            "ProtoCraft_THT:Terminal_2P_5.08mm",
            5_080_000,
            1_500_000,
            10_000_000,
        ),
        Point::new(0, 0),
        "J1-1 +12V",
    );

    let resistor_fp = catalog
        .footprints
        .get("Resistor_SMD:R_0805_2012Metric")
        .cloned()
        .ok_or_else(|| eda_i18n::text("O catálogo não contém footprint de resistor 0805."))?;
    let capacitor_fp = catalog
        .footprints
        .get("Capacitor_SMD:C_0805_2012Metric")
        .or_else(|| catalog.footprints.get("Resistor_SMD:R_0805_2012Metric"))
        .cloned()
        .ok_or_else(|| eda_i18n::text("O catálogo não contém footprint SMD 0805."))?;
    let output_cap_fp = power_output_capacitor_footprint(&source);
    let inductor_fp = power_inductor_footprint(&source);
    let bulk_fp = two_pin_through_hole(
        &source,
        "ProtoCraft_THT:Electrolytic_10mm_P5mm",
        5_000_000,
        1_000_000,
        10_000_000,
    );
    let terminal_fp = two_pin_through_hole(
        &source,
        "ProtoCraft_THT:Terminal_2P_5.08mm",
        5_080_000,
        1_500_000,
        10_000_000,
    );

    add_two_pin_device(
        &mut project,
        ComponentId(9),
        "RBOOT",
        &eda_i18n::text("Jumper de 0 Ω para o modo de condução eficiente indicado pela TI"),
        "ProtoCraft_SMD:Jumper_0805",
        0.0,
        resistor_fp.clone(),
        Point::new(0, 0),
        "",
    );
    let mut add = |id: u64, reference: &str, kind: ComponentKind, value: f64, fp: Footprint| {
        add_component(
            &mut project,
            ComponentId(id),
            reference,
            kind,
            value,
            Point::new(0, 0),
            fp,
        );
    };
    add(4, "L1", ComponentKind::Inductor, 4.7e-6, inductor_fp);
    add(
        5,
        "RFBT",
        ComponentKind::Resistor,
        100_000.0,
        resistor_fp.clone(),
    );
    add(
        6,
        "RFBB",
        ComponentKind::Resistor,
        24_300.0,
        resistor_fp.clone(),
    );
    add(
        7,
        "RT",
        ComponentKind::Resistor,
        33_200.0,
        resistor_fp.clone(),
    );
    add(
        8,
        "REN",
        ComponentKind::Resistor,
        100_000.0,
        resistor_fp.clone(),
    );
    add(
        10,
        "CFF",
        ComponentKind::Capacitor,
        22e-12,
        capacitor_fp.clone(),
    );
    add(
        11,
        "CBOOT",
        ComponentKind::Capacitor,
        100e-9,
        capacitor_fp.clone(),
    );
    add(
        12,
        "CVCC",
        ComponentKind::Capacitor,
        1e-6,
        capacitor_fp.clone(),
    );
    add(
        13,
        "CBIAS",
        ComponentKind::Capacitor,
        100e-9,
        capacitor_fp.clone(),
    );
    add(
        14,
        "CIN1",
        ComponentKind::Capacitor,
        4.7e-6,
        output_cap_fp.clone(),
    );
    add(
        15,
        "CIN2",
        ComponentKind::Capacitor,
        4.7e-6,
        output_cap_fp.clone(),
    );
    add(
        16,
        "CIN3",
        ComponentKind::Capacitor,
        100e-9,
        capacitor_fp.clone(),
    );
    add(
        17,
        "CIN4",
        ComponentKind::Capacitor,
        100e-9,
        capacitor_fp.clone(),
    );
    add(
        18,
        "CBULK_IN",
        ComponentKind::Capacitor,
        47e-6,
        bulk_fp.clone(),
    );
    add(
        19,
        "COUT1",
        ComponentKind::Capacitor,
        22e-6,
        output_cap_fp.clone(),
    );
    add(
        20,
        "COUT2",
        ComponentKind::Capacitor,
        22e-6,
        output_cap_fp.clone(),
    );
    add(
        21,
        "COUT3",
        ComponentKind::Capacitor,
        22e-6,
        output_cap_fp.clone(),
    );
    add(22, "COUT4", ComponentKind::Capacitor, 22e-6, output_cap_fp);
    add(23, "COUT_BULK", ComponentKind::Capacitor, 560e-6, bulk_fp);
    add_two_pin_device(
        &mut project,
        ComponentId(24),
        "J_OUT",
        &eda_i18n::text("Saída auxiliar 5,1 V / GND; não alimentar em paralelo com USB-C"),
        "ProtoCraft_THT:Terminal_2P_5.08mm",
        5.1,
        terminal_fp,
        Point::new(0, 0),
        "J_OUT-1 +5V1",
    );

    // A fixture de PCB posiciona componentes separadamente; o esquema também
    // precisa de coordenadas únicas para que rótulos de pinos não se toquem.
    for (id, x, y) in [
        (1, -60, 0),
        (2, 0, 0),
        (3, -40, 40),
        (4, 24, 0),
        (5, 24, 20),
        (6, 48, 20),
        (7, 48, 0),
        (8, 48, -20),
        (9, 72, -20),
        (10, 72, 20),
        (11, 72, 0),
        (12, 96, 20),
        (13, 96, 0),
        (14, 120, 20),
        (15, 144, 20),
        (16, 120, 0),
        (17, 144, 0),
        (18, 168, 20),
        (19, 168, 0),
        (20, 192, 20),
        (21, 192, 0),
        (22, 216, 20),
        (23, 216, 0),
        (24, 240, 0),
    ] {
        let component = project
            .components
            .get_mut(&ComponentId(id))
            .ok_or_else(|| {
                eda_i18n::format(
                    "Componente {id} ausente ao posicionar o esquema.",
                    &[("id", id.to_string())],
                )
            })?;
        component.position = Point::new(x, y);
    }

    set_pin_label(&mut project, ComponentId(3), 0, "VIN12")?;
    set_pin_label(&mut project, ComponentId(3), 1, "GND")?;
    set_pin_label(&mut project, ComponentId(24), 0, "5V1")?;
    set_pin_label(&mut project, ComponentId(24), 1, "GND")?;
    for (pin, net) in [
        (0, "5V1"),
        (1, "VCC"),
        (2, "GND"),
        (3, "FB"),
        (4, "PGOOD"),
        (5, "RT"),
        (6, "EN"),
        (7, "VIN12"),
        (8, "GND"),
        (9, "SW"),
        (10, "GND"),
        (11, "VIN12"),
        (12, "RBOOT_N"),
        (13, "CBOOT_N"),
    ] {
        set_pin_label(&mut project, ComponentId(2), pin, net)?;
    }
    for (component, pin, net) in [
        (4, 0, "SW"),
        (4, 1, "5V1"),
        (5, 0, "5V1"),
        (5, 1, "FB"),
        (6, 0, "FB"),
        (6, 1, "GND"),
        (7, 0, "RT"),
        (7, 1, "GND"),
        (8, 0, "VIN12"),
        (8, 1, "EN"),
        (9, 0, "RBOOT_N"),
        (9, 1, "CBOOT_N"),
        (10, 0, "5V1"),
        (10, 1, "FB"),
        (11, 0, "CBOOT_N"),
        (11, 1, "SW"),
        (12, 0, "VCC"),
        (12, 1, "GND"),
        (13, 0, "5V1"),
        (13, 1, "GND"),
        (14, 0, "VIN12"),
        (14, 1, "GND"),
        (15, 0, "VIN12"),
        (15, 1, "GND"),
        (16, 0, "VIN12"),
        (16, 1, "GND"),
        (17, 0, "VIN12"),
        (17, 1, "GND"),
        (18, 0, "VIN12"),
        (18, 1, "GND"),
        (19, 0, "5V1"),
        (19, 1, "GND"),
        (20, 0, "5V1"),
        (20, 1, "GND"),
        (21, 0, "5V1"),
        (21, 1, "GND"),
        (22, 0, "5V1"),
        (22, 1, "GND"),
        (23, 0, "5V1"),
        (23, 1, "GND"),
    ] {
        set_pin_label(&mut project, ComponentId(component), pin, net)?;
    }
    for pin in 0..40u16 {
        let signal = project.components[&ComponentId(1)].pin_name(pin).to_owned();
        let net = match (pin, signal.as_str()) {
            (1 | 3, "5V") => "5V1".to_owned(),
            (_, "GND") => "GND".to_owned(),
            _ => format!("PI_PIN_{}", pin + 1),
        };
        set_pin_label(&mut project, ComponentId(1), pin, &net)?;
    }

    let mut board = Board {
        width_nm: 65 * eda_core::NM_PER_MM,
        height_nm: 56_500_000,
        corner_radius_nm: 3 * eda_core::NM_PER_MM,
        thickness_nm: 1_600_000,
        clearance_nm: 100_000,
        edge_clearance_nm: 500_000,
        placements: BTreeMap::new(),
        tracks: BTreeMap::new(),
        vias: BTreeMap::new(),
        holes: BTreeMap::new(),
        net_classes: BTreeMap::new(),
    };
    let coordinates = [
        (1, 8_370_000, 5_000_000),
        (2, 29_000_000, 25_000_000),
        (3, 57_000_000, 27_000_000),
        (4, 21_000_000, 22_000_000),
        (5, 28_000_000, 36_000_000),
        (6, 32_000_000, 36_000_000),
        (7, 34_000_000, 18_000_000),
        (8, 34_000_000, 14_000_000),
        (9, 28_000_000, 34_000_000),
        (10, 28_000_000, 39_000_000),
        (11, 33_000_000, 33_000_000),
        (12, 41_000_000, 25_000_000),
        (13, 37_000_000, 20_000_000),
        (14, 35_000_000, 30_000_000),
        (15, 40_000_000, 30_000_000),
        (16, 35_000_000, 24_000_000),
        (17, 35_000_000, 27_000_000),
        (18, 50_000_000, 32_000_000),
        (19, 7_000_000, 15_000_000),
        (20, 13_000_000, 15_000_000),
        (21, 7_000_000, 19_000_000),
        (22, 13_000_000, 19_000_000),
        (23, 21_000_000, 30_000_000),
        (24, 47_000_000, 45_000_000),
    ];
    for (id, x, y) in coordinates {
        board.placements.insert(
            ComponentId(id),
            Placement {
                position: NmPoint::new(x, y),
                rotation: if id == 2 || id == 4 { 2 } else { 0 },
                side: BoardSide::Top,
            },
        );
    }
    for (id, x, y) in [
        (1, 3_500_000, 3_500_000),
        (2, 61_500_000, 3_500_000),
        (3, 3_500_000, 52_500_000),
        (4, 61_500_000, 52_500_000),
    ] {
        board.holes.insert(
            id,
            MountingHole {
                id,
                position: NmPoint::new(x, y),
                drill_nm: 2_750_000,
            },
        );
    }
    insert_net_class(&mut board, "INPUT_12V", &["VIN12"], 800_000, 100_000);
    insert_net_class(&mut board, "OUTPUT_3A", &["5V1", "GND"], 800_000, 100_000);
    insert_net_class(&mut board, "SWITCH_NODE", &["SW"], 800_000, 100_000);
    insert_net_class(
        &mut board,
        "CONTROL",
        &["FB", "VCC", "RT", "EN", "PGOOD", "RBOOT_N", "CBOOT_N"],
        250_000,
        100_000,
    );
    project.board = Some(Box::new(board));
    add_controller_breakouts(&mut project)?;
    add_input_capacitor_vin_bridge(&mut project)?;
    add_pi_power_buses(&mut project)?;
    project.validate().map_err(|error| error.to_string())?;
    Ok(project)
}

fn add_input_capacitor_vin_bridge(project: &mut Project) -> Result<(), String> {
    let net = eda_netlist::build_netlist(project);
    let board = project
        .board
        .as_mut()
        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
    let mut next_track_id = board
        .tracks
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
    let mut next_via_id = board
        .vias
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de IDs de via atingido."))?;
    let pins = [
        PinRef {
            component: ComponentId(14),
            pin: 0,
        },
        PinRef {
            component: ComponentId(15),
            pin: 0,
        },
    ];
    let mut vias = Vec::new();
    for pin in pins {
        let component = project
            .components
            .get(&pin.component)
            .ok_or_else(|| eda_i18n::text("Capacitor de entrada ausente."))?;
        let pad = component
            .footprint
            .as_ref()
            .and_then(|footprint| footprint.pads.iter().find(|pad| pad.number == "1"))
            .ok_or_else(|| eda_i18n::text("Pad VIN do capacitor de entrada ausente."))?;
        let placement = board
            .placements
            .get(&pin.component)
            .copied()
            .ok_or_else(|| eda_i18n::text("Capacitor de entrada não está posicionado."))?;
        let pad_position = placement.transform(pad.position);
        let via_position = NmPoint::new(pad_position.x, pad_position.y + 1_500_000);
        board.tracks.insert(
            next_track_id,
            Track {
                id: next_track_id,
                start: pad_position,
                end: via_position,
                width_nm: 250_000,
                layer: BoardSide::Top,
                net: pin,
            },
        );
        next_track_id += 1;
        board.vias.insert(
            next_via_id,
            eda_core::Via {
                id: next_via_id,
                position: via_position,
                diameter_nm: 400_000,
                drill_nm: 200_000,
                net: pin,
            },
        );
        next_via_id += 1;
        vias.push((pin, via_position));
    }
    let [(first_pin, first), (second_pin, second)] = vias.as_slice() else {
        return Err(eda_i18n::text("Não foi possível criar as vias de entrada."));
    };
    if net.net_for_pin(*first_pin) != net.net_for_pin(*second_pin) {
        return Err(eda_i18n::text(
            "Os capacitores de entrada não compartilham VIN.",
        ));
    }
    board.tracks.insert(
        next_track_id,
        Track {
            id: next_track_id,
            start: *first,
            end: *second,
            width_nm: 800_000,
            layer: BoardSide::Bottom,
            net: *first_pin,
        },
    );
    Ok(())
}

fn add_controller_breakouts(project: &mut Project) -> Result<(), String> {
    let component_id = ComponentId(2);
    let component = project
        .components
        .get(&component_id)
        .ok_or_else(|| eda_i18n::text("O controlador buck não foi encontrado."))?;
    let footprint = component
        .footprint
        .as_ref()
        .ok_or_else(|| eda_i18n::text("O controlador buck não tem footprint."))?;
    let placement = project
        .board
        .as_ref()
        .and_then(|board| board.placements.get(&component_id))
        .copied()
        .ok_or_else(|| eda_i18n::text("O controlador buck não está posicionado na PCB."))?;
    let pin_nets = [
        "5V1", "VCC", "GND", "FB", "PGOOD", "RT", "EN", "VIN12", "GND", "SW", "GND", "VIN12",
        "RBOOT_N", "CBOOT_N",
    ];
    let board = project
        .board
        .as_mut()
        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
    let mut next_track_id = board
        .tracks
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
    let mut next_via_id = board
        .vias
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de IDs de via atingido."))?;
    let mut via_positions = [NmPoint::new(0, 0); 14];

    for (index, net_name) in pin_nets.iter().enumerate() {
        if *net_name == "PGOOD" || *net_name == "SW" {
            continue;
        }
        let pad_number = (index + 1).to_string();
        let pad = footprint
            .pads
            .iter()
            .find(|pad| pad.number == pad_number)
            .ok_or_else(|| {
                eda_i18n::format(
                    "O footprint do controlador não contém o pad {pad}.",
                    &[("pad", pad_number.clone())],
                )
            })?;
        let (dx, dy) = if pad.position.x.abs() >= pad.position.y.abs() {
            (pad.position.x.signum(), 0)
        } else {
            (0, pad.position.y.signum())
        };
        // O escape curto leva cada pino de 0,5 mm de passo à face inferior,
        // onde as redes podem atravessar o encapsulamento sem cruzar outros pads.
        let escape_length = if matches!(index, 0 | 2 | 6 | 12) {
            1_650_000
        } else {
            650_000
        };
        let local_via = NmPoint::new(
            pad.position.x + dx * escape_length,
            pad.position.y + dy * escape_length,
        );
        let start = placement.transform(pad.position);
        let via_position = placement.transform(local_via);
        via_positions[index] = via_position;
        let pin = PinRef {
            component: component_id,
            pin: index as u16,
        };
        let track_width_nm = board
            .net_classes
            .values()
            .find(|rules| rules.net_names.contains(*net_name))
            .map_or(250_000, |rules| {
                rules
                    .pad_escape
                    .as_ref()
                    .map_or(rules.track_width_nm, |escape| escape.track_width_nm)
            });
        board.tracks.insert(
            next_track_id,
            Track {
                id: next_track_id,
                start,
                end: via_position,
                width_nm: track_width_nm,
                layer: BoardSide::Top,
                net: pin,
            },
        );
        next_track_id = next_track_id
            .checked_add(1)
            .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
        board.vias.insert(
            next_via_id,
            eda_core::Via {
                id: next_via_id,
                position: via_position,
                diameter_nm: 400_000,
                drill_nm: 200_000,
                net: pin,
            },
        );
        next_via_id = next_via_id
            .checked_add(1)
            .ok_or_else(|| eda_i18n::text("Limite de IDs de via atingido."))?;
    }

    // VIN e GND aparecem em faces opostas do QFN. Pontes curtas na face inferior
    // unem esses pinos antes que as conexões mais longas sejam autorroteadas.
    let bridges = [
        (7, "VIN12", vec![via_positions[7], via_positions[11]]),
        (
            2,
            "GND",
            [
                NmPoint::new(-3_300_000, 0),
                NmPoint::new(-4_000_000, 0),
                NmPoint::new(-4_000_000, 4_500_000),
                NmPoint::new(3_500_000, 4_500_000),
                NmPoint::new(3_500_000, 1_000_000),
                NmPoint::new(2_300_000, 1_000_000),
            ]
            .into_iter()
            .map(|point| placement.transform(point))
            .collect(),
        ),
        (
            8,
            "GND",
            [
                NmPoint::new(2_300_000, 1_000_000),
                NmPoint::new(2_300_000, -1_000_000),
            ]
            .into_iter()
            .map(|point| placement.transform(point))
            .collect(),
        ),
    ];
    for (start_pin, net_name, points) in bridges {
        let track_width_nm = board
            .net_classes
            .values()
            .find(|rules| rules.net_names.contains(net_name))
            .map_or(250_000, |rules| rules.track_width_nm);
        let pin = PinRef {
            component: component_id,
            pin: start_pin as u16,
        };
        for pair in points.windows(2) {
            board.tracks.insert(
                next_track_id,
                Track {
                    id: next_track_id,
                    start: pair[0],
                    end: pair[1],
                    width_nm: track_width_nm,
                    layer: BoardSide::Bottom,
                    net: pin,
                },
            );
            next_track_id = next_track_id
                .checked_add(1)
                .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
        }
    }
    Ok(())
}

fn insert_net_class(board: &mut Board, name: &str, nets: &[&str], width: i64, clearance: i64) {
    board.net_classes.insert(
        name.into(),
        NetClass {
            net_names: nets
                .iter()
                .map(|net| (*net).into())
                .collect::<BTreeSet<_>>(),
            clearance_nm: clearance,
            track_width_nm: width,
            via_diameter_nm: if width >= 800_000 { 800_000 } else { 400_000 },
            via_drill_nm: if width >= 800_000 { 400_000 } else { 200_000 },
            pad_escape: (width >= 800_000).then_some(eda_core::PadEscapeRule {
                track_width_nm: 250_000,
                max_length_nm: 2_000_000,
                via_diameter_nm: 400_000,
                via_drill_nm: 200_000,
            }),
        },
    );
}

fn add_pi_power_buses(project: &mut Project) -> Result<(), String> {
    let header = project
        .components
        .get(&ComponentId(1))
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let device = header
        .device
        .as_ref()
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let footprint = header
        .footprint
        .as_ref()
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let board = project
        .board
        .as_mut()
        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
    let placement = board
        .placements
        .get(&ComponentId(1))
        .copied()
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let mut ground = Vec::new();
    let mut output = Vec::new();
    for (index, pin) in device
        .pins
        .iter()
        .enumerate()
        .filter(|(_, pin)| matches!(pin.name.as_str(), "GND" | "5V"))
    {
        let pad = footprint
            .pads
            .iter()
            .find(|pad| pin.pads.contains(&pad.number))
            .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
        let entry = (
            PinRef {
                component: ComponentId(1),
                pin: index as u16,
            },
            placement.transform(pad.position),
        );
        if pin.name == "GND" {
            ground.push(entry);
        } else {
            output.push(entry);
        }
    }
    let ground_anchor = ground
        .first()
        .map(|entry| entry.0)
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let output_anchor = output
        .first()
        .map(|entry| entry.0)
        .ok_or_else(|| eda_i18n::text("Header Raspberry Pi ausente."))?;
    let mut next_id = board
        .tracks
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
    let mut path = |net, points: &[NmPoint]| -> Result<(), String> {
        for pair in points.windows(2).filter(|pair| pair[0] != pair[1]) {
            board.tracks.insert(
                next_id,
                Track {
                    id: next_id,
                    start: pair[0],
                    end: pair[1],
                    width_nm: 800_000,
                    layer: BoardSide::Bottom,
                    net,
                },
            );
            next_id = next_id
                .checked_add(1)
                .ok_or_else(|| eda_i18n::text("Limite de IDs de trilha atingido."))?;
        }
        Ok(())
    };
    // As duas fileiras escapam para lados opostos, evitando atravessar GPIOs.
    let upper_y = 2_000_000;
    let lower_y = 10_500_000;
    let join_x = 58_500_000;
    let mut upper_start = join_x;
    let mut lower_start = join_x;
    for (net, position) in ground {
        let bus_y = if position.y == placement.position.y {
            upper_y
        } else {
            lower_y
        };
        if bus_y == upper_y {
            upper_start = upper_start.min(position.x);
        } else {
            lower_start = lower_start.min(position.x);
        }
        path(net, &[position, NmPoint::new(position.x, bus_y)])?;
    }
    path(
        ground_anchor,
        &[
            NmPoint::new(upper_start, upper_y),
            NmPoint::new(join_x, upper_y),
            NmPoint::new(join_x, lower_y),
            NmPoint::new(lower_start, lower_y),
        ],
    )?;
    let output_points: Vec<_> = output.into_iter().map(|entry| entry.1).collect();
    path(output_anchor, &output_points)?;
    Ok(())
}

fn add_device(
    project: &mut Project,
    id: ComponentId,
    reference: &str,
    device: DeviceDefinition,
    footprint: Footprint,
    position: Point,
    value: f64,
) {
    project.components.insert(
        id,
        Component {
            id,
            reference: reference.into(),
            kind: ComponentKind::Device,
            value,
            position,
            rotation: 0,
            imported: None,
            device: Some(Box::new(device)),
            footprint: Some(Box::new(footprint)),
            parameters: Default::default(),
        },
    );
    project.next_id = project.next_id.max(id.0 + 1);
}

fn add_component(
    project: &mut Project,
    id: ComponentId,
    reference: &str,
    kind: ComponentKind,
    value: f64,
    position: Point,
    footprint: Footprint,
) {
    project.components.insert(
        id,
        Component {
            id,
            reference: reference.into(),
            kind,
            value,
            position,
            rotation: 0,
            imported: None,
            device: None,
            footprint: Some(Box::new(footprint)),
            parameters: Default::default(),
        },
    );
    project.next_id = project.next_id.max(id.0 + 1);
}

fn add_two_pin_device(
    project: &mut Project,
    id: ComponentId,
    reference: &str,
    description: &str,
    footprint_id: &str,
    value: f64,
    footprint: Footprint,
    position: Point,
    _label: &str,
) {
    let source = footprint.source.clone();
    let device = DeviceDefinition {
        id: reference.into(),
        requested_part: reference.into(),
        manufacturer: "ProtoCraft".into(),
        family: description.into(),
        form: footprint_id.into(),
        description: description.into(),
        datasheet: String::new(),
        pins: vec![
            DevicePin {
                number: "1".into(),
                name: "1".into(),
                electrical_type: "passive".into(),
                offset: Point::new(-3, 0),
                pads: vec!["1".into()],
            },
            DevicePin {
                number: "2".into(),
                name: "2".into(),
                electrical_type: "passive".into(),
                offset: Point::new(3, 0),
                pads: vec!["2".into()],
            },
        ],
        source,
    };
    add_device(project, id, reference, device, footprint, position, value);
}

fn set_pin_label(
    project: &mut Project,
    component: ComponentId,
    pin: u16,
    name: &str,
) -> Result<(), String> {
    let position = project
        .components
        .get(&component)
        .and_then(|part| part.pins().get(pin as usize).map(|pin| pin.position))
        .ok_or_else(|| {
            eda_i18n::format(
                "Pino {pin} ausente no componente {component}.",
                &[
                    ("pin", pin.to_string()),
                    ("component", component.0.to_string()),
                ],
            )
        })?;
    let id = LabelId(project.next_id);
    project.next_id = project
        .next_id
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Limite de identificadores do projeto atingido."))?;
    project.labels.insert(
        id,
        eda_core::NetLabel {
            id,
            position,
            name: name.into(),
        },
    );
    Ok(())
}

fn two_pin_through_hole(
    source: &DataSource,
    id: &str,
    spacing: i64,
    drill: i64,
    body_width: i64,
) -> Footprint {
    let pad_size = drill + 1_200_000;
    let pads = [(-spacing / 2, "1"), (spacing / 2, "2")]
        .into_iter()
        .map(|(x, number)| Pad {
            number: number.into(),
            position: NmPoint::new(x, 0),
            size: NmPoint::new(pad_size, pad_size),
            rotation_mdeg: 0,
            shape: PadShape::Circle,
            technology: PadTechnology::ThroughHole,
            drill: Some(NmPoint::new(drill, drill)),
            layers: vec!["*.Cu".into(), "F.Mask".into()],
            roundrect_ratio_ppm: 0,
            mask_margin_nm: 0,
            paste_margin_nm: 0,
            paste_ratio_ppm: 0,
            polygons: vec![],
        })
        .collect();
    Footprint {
        id: id.into(),
        description: "Geometria genérica de dois pads; confirmar componente físico antes da compra"
            .into(),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-body_width / 2, -2_000_000),
            max: NmPoint::new(body_width / 2, 2_000_000),
        },
        pads,
        lines: rectangle_lines(
            -body_width / 2,
            -2_000_000,
            body_width / 2,
            2_000_000,
            "F.Fab",
            100_000,
        ),
        keepouts: vec![],
        source: source.clone(),
    }
}

fn power_inductor_footprint(source: &DataSource) -> Footprint {
    let pads = [(-3_500_000, "1"), (3_500_000, "2")]
        .into_iter()
        .map(|(x, number)| Pad {
            number: number.into(),
            position: NmPoint::new(x, 0),
            size: NmPoint::new(2_800_000, 4_000_000),
            rotation_mdeg: 0,
            shape: PadShape::RoundRect,
            technology: PadTechnology::Smd,
            drill: None,
            layers: vec!["F.Cu".into(), "F.Mask".into(), "F.Paste".into()],
            roundrect_ratio_ppm: 200_000,
            mask_margin_nm: 0,
            paste_margin_nm: 0,
            paste_ratio_ppm: 0,
            polygons: vec![],
        })
        .collect();
    Footprint {
        id: "ProtoCraft_Power:Inductor_10x10mm_XAL1010_concept".into(),
        description: eda_i18n::text(
            "Área conceitual 10 × 10 mm para Coilcraft XAL1010-472MED; revisar land pattern do fabricante.",
        ),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-5_000_000, -5_000_000),
            max: NmPoint::new(5_000_000, 5_000_000),
        },
        pads,
        lines: rectangle_lines(
            -5_000_000, -5_000_000, 5_000_000, 5_000_000, "F.Fab", 100_000,
        ),
        keepouts: vec![],
        source: source.clone(),
    }
}

fn power_output_capacitor_footprint(source: &DataSource) -> Footprint {
    let pads = [(-1_500_000, "1"), (1_500_000, "2")]
        .into_iter()
        .map(|(x, number)| Pad {
            number: number.into(),
            position: NmPoint::new(x, 0),
            size: NmPoint::new(1_400_000, 1_900_000),
            rotation_mdeg: 0,
            shape: PadShape::RoundRect,
            technology: PadTechnology::Smd,
            drill: None,
            layers: vec!["F.Cu".into(), "F.Mask".into(), "F.Paste".into()],
            roundrect_ratio_ppm: 200_000,
            mask_margin_nm: 0,
            paste_margin_nm: 0,
            paste_ratio_ppm: 0,
            polygons: vec![],
        })
        .collect();
    Footprint {
        id: "ProtoCraft_SMD:C_1210_PowerConcept".into(),
        description: eda_i18n::text(
            "Geometria conceitual 1210 para cerâmicos de potência; selecione capacitância efetiva sob polarização e confira o datasheet do componente.",
        ),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds {
            min: NmPoint::new(-1_700_000, -1_250_000),
            max: NmPoint::new(1_700_000, 1_250_000),
        },
        pads,
        lines: rectangle_lines(
            -1_700_000, -1_250_000, 1_700_000, 1_250_000, "F.Fab", 100_000,
        ),
        keepouts: vec![],
        source: source.clone(),
    }
}

fn rectangle_lines(
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
    layer: &str,
    width: i64,
) -> Vec<FootprintLine> {
    [
        (NmPoint::new(min_x, min_y), NmPoint::new(max_x, min_y)),
        (NmPoint::new(max_x, min_y), NmPoint::new(max_x, max_y)),
        (NmPoint::new(max_x, max_y), NmPoint::new(min_x, max_y)),
        (NmPoint::new(min_x, max_y), NmPoint::new(min_x, min_y)),
    ]
    .into_iter()
    .map(|(start, end)| FootprintLine {
        start,
        end,
        width_nm: width,
        layer: layer.into(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_netlist::build_netlist;

    #[test]
    fn pi4_buck_calculation_matches_documented_design_targets() {
        let result = BuckDesignSpec::default()
            .calculate()
            .expect("parâmetros padrão válidos");
        assert!((result.duty_cycle_ideal - 0.425).abs() < 1e-12);
        assert!((result.output_power_w - 15.3).abs() < 1e-12);
        assert!((result.input_current_estimate_a - 1.38587).abs() < 0.001);
        assert!((result.minimum_inductance_h - 4.8875e-6).abs() < 1e-10);
        assert!((result.output_voltage_nominal_v - 5.115226).abs() < 0.001);
        assert!(result.inductor_peak_current_at_load_a < 4.0);
    }

    #[test]
    fn buck_calculation_rejects_non_finite_and_invalid_ranges() {
        let invalid = BuckDesignSpec {
            input_voltage_v: f64::NAN,
            ..Default::default()
        };
        assert!(invalid.calculate().is_err());
        let invalid = BuckDesignSpec {
            output_current_a: 7.0,
            ..Default::default()
        };
        assert!(invalid.calculate().is_err());
        let invalid = BuckDesignSpec {
            input_voltage_v: 48.0,
            ..Default::default()
        };
        assert!(invalid.calculate().is_err());
        let invalid = BuckDesignSpec {
            switching_frequency_hz: 150_000.0,
            ..Default::default()
        };
        assert!(invalid.calculate().is_err());
    }

    #[test]
    fn lm61460_definition_maps_all_pins_to_ti_land_pattern_numbers() {
        let (device, footprint) = lm61460_definition();
        assert_eq!(device.pins.len(), 14);
        assert_eq!(footprint.pads.len(), 14);
        for (index, pin) in device.pins.iter().enumerate() {
            assert_eq!(pin.number, (index + 1).to_string());
            assert_eq!(pin.pads, vec![pin.number.clone()]);
            assert_eq!(footprint.pads[index].number, pin.number);
        }
    }

    #[test]
    fn pi4_buck_project_has_mechanical_profile_and_correct_power_nets() {
        let project = buck_pi4_project().expect("fixture válida");
        project.validate().expect("invariantes do projeto");
        let board = project.board.as_ref().expect("placa presente");
        assert_eq!((board.width_nm, board.height_nm), (65_000_000, 56_500_000));
        assert_eq!(board.holes.len(), 4);
        assert_eq!(board.placements.len(), project.components.len());
        let netlist = build_netlist(&project);
        let net_for = |component, pin| {
            netlist
                .net_for_pin(eda_core::PinRef {
                    component: ComponentId(component),
                    pin,
                })
                .unwrap()
        };
        assert_eq!(net_for(1, 1), net_for(1, 3));
        assert_eq!(net_for(1, 1), net_for(4, 1));
        assert_eq!(net_for(1, 5), net_for(1, 8));
        assert_ne!(net_for(1, 1), net_for(1, 5));
        assert_eq!(net_for(3, 0), net_for(2, 7));
        assert_ne!(net_for(3, 0), net_for(1, 1));
        assert_eq!(net_for(3, 1), net_for(1, 5));
        assert_eq!(net_for(2, 9), net_for(4, 0));
        assert_ne!(net_for(2, 9), net_for(1, 1));
        assert_ne!(net_for(2, 3), net_for(1, 1));
        assert!(netlist.nets.len() >= 30);
        assert!(!board.net_classes.is_empty());
    }
}
