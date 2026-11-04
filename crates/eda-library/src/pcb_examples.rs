use eda_core::{FORMAT_VERSION, NetClass, Project};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
pub struct PcbExampleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

const BASIC: &str = include_str!("../../../docs/examples/pcb-trilhas-furos.protocraft");
const TIMER_555: &str = include_str!("../../../examples/555-led-1hz-pcb.protocraft");
const PI4_SHIELD: &str = include_str!("../../../examples/raspberry-pi-4-shield.protocraft");
const PI4_BUCK: &str = include_str!("../../../examples/pcb-pi4-12v-buck.protocraft");

const CATALOG: &[PcbExampleInfo] = &[
    PcbExampleInfo {
        id: "pcb-trilhas-furos",
        name: "Básico · trilhas, vias e furos",
        description: "Placa compacta para explorar cobre, vias, furos e uma classe de rede.",
    },
    PcbExampleInfo {
        id: "pcb-555-led-1hz",
        name: "Intermediário · pisca-pisca 555 roteado",
        description: "Projeto completo em duas camadas, com regras separadas para alimentação e sinal.",
    },
    PcbExampleInfo {
        id: "pcb-stm32-esp32",
        name: "Avançado · STM32 + ESP32-S3",
        description: "Layout inicial de dois módulos com classes de rede e conexões pendentes para experimentar o autorroteador.",
    },
    PcbExampleInfo {
        id: "pcb-raspberry-pi-4-shield",
        name: "Complexo · shield Raspberry Pi 4",
        description: "Placa de 85 × 56 mm com header GPIO, furos e indicadores de alimentação.",
    },
    PcbExampleInfo {
        id: "pcb-pi4-12v-buck",
        name: "Avançado · buck 12 V para Raspberry Pi 4",
        description: "Fonte LM61460 5,1 V / 3 A nominal, saída no GPIO, rede de 5 V com classe de 0,8 mm e DRC limpo.",
    },
];

pub fn pcb_example_catalog() -> &'static [PcbExampleInfo] {
    CATALOG
}

/// Abre uma fixture nativa embarcada ou constrói o exemplo físico parametrizado.
pub fn load_pcb_example(id: &str) -> Result<Project, String> {
    let mut project = match id {
        "pcb-trilhas-furos" => decode(BASIC)?,
        "pcb-555-led-1hz" => decode(TIMER_555)?,
        "pcb-raspberry-pi-4-shield" => decode(PI4_SHIELD)?,
        "pcb-pi4-12v-buck" => decode(PI4_BUCK)?,
        "pcb-stm32-esp32" => crate::physical_example()?,
        _ => {
            return Err(eda_i18n::format(
                "Exemplo de PCB desconhecido: {id}.",
                &[("id", id.to_owned())],
            ));
        }
    };
    add_demonstration_classes(id, &mut project)?;
    project.validate().map_err(|error| error.to_string())?;
    Ok(project)
}

fn decode(source: &str) -> Result<Project, String> {
    let mut project: Project = serde_json::from_str(source).map_err(|error| error.to_string())?;
    // As fixtures embarcadas podem ser anteriores à versão que adicionou net classes opcionais.
    if project.format_version < FORMAT_VERSION {
        project.format_version = FORMAT_VERSION;
    }
    project.validate().map_err(|error| error.to_string())?;
    Ok(project)
}

fn add_demonstration_classes(id: &str, project: &mut Project) -> Result<(), String> {
    let board = project
        .board
        .as_mut()
        .ok_or_else(|| "A fixture de PCB não contém uma placa.".to_owned())?;
    let clearance_nm = board.clearance_nm;
    let mut add = |name: &str, nets: &[&str], width_nm: i64| {
        board.net_classes.insert(
            name.into(),
            NetClass {
                net_names: nets
                    .iter()
                    .map(|net| (*net).to_owned())
                    .collect::<BTreeSet<_>>(),
                clearance_nm,
                track_width_nm: width_nm,
                via_diameter_nm: 600_000,
                via_drill_nm: 300_000,
                pad_escape: None,
            },
        );
    };
    match id {
        "pcb-trilhas-furos" => add("SINAL", &["SIGNAL"], 250_000),
        "pcb-555-led-1hz" => {
            add("ALIMENTACAO", &["VCC"], 350_000);
            add("SINAIS", &["CAP", "CTRL", "DIS", "LED_K", "VOUT"], 250_000);
        }
        "pcb-stm32-esp32" => add("ALIMENTACAO", &["3V3", "0"], 400_000),
        "pcb-raspberry-pi-4-shield" => add("ALIMENTACAO", &["5V", "3V3"], 250_000),
        "pcb-pi4-12v-buck" => {}
        _ => {
            return Err(eda_i18n::format(
                "Exemplo de PCB desconhecido: {id}.",
                &[("id", id.to_owned())],
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_pcb_examples_open_as_valid_native_projects() {
        for example in pcb_example_catalog() {
            let project = load_pcb_example(example.id)
                .unwrap_or_else(|error| panic!("{}: {error}", example.id));
            assert_eq!(project.format_version, FORMAT_VERSION);
            assert!(project.board.is_some());
            assert!(!project.board.as_ref().unwrap().net_classes.is_empty());
        }
    }
}
