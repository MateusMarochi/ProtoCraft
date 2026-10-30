//! Esquemas JSON públicos para automação CLI/MCP.
use serde_json::{Value, json};

fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type":"object",
        "properties":properties,
        "required":required,
        "additionalProperties":false
    })
}

fn integer(minimum: i64, maximum: i64) -> Value {
    json!({"type":"integer","minimum":minimum,"maximum":maximum})
}

fn positive_id() -> Value {
    json!({"type":"integer","minimum":1})
}

fn string(max_length: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max_length})
}

fn point() -> Value {
    object(
        json!({"x":integer(-1_000_000,1_000_000),"y":integer(-1_000_000,1_000_000)}),
        &["x", "y"],
    )
}

fn nm_point() -> Value {
    object(
        json!({"x":integer(-2_000_000_000,2_000_000_000),"y":integer(-2_000_000_000,2_000_000_000)}),
        &["x", "y"],
    )
}

fn pin_ref() -> Value {
    object(
        json!({"component":positive_id(),"pin":integer(0,u16::MAX.into())}),
        &["component", "pin"],
    )
}

fn placement() -> Value {
    object(
        json!({
            "position":nm_point(),
            "rotation":integer(0,3),
            "side":{"type":"string","enum":["Top","Bottom"]}
        }),
        &["position", "rotation"],
    )
}

fn track() -> Value {
    object(
        json!({
            "id":positive_id(),
            "start":nm_point(),
            "end":nm_point(),
            "width_nm":integer(1,10_000_000),
            "layer":{"type":"string","enum":["Top","Bottom"]},
            "net":pin_ref()
        }),
        &["id", "start", "end", "width_nm", "layer", "net"],
    )
}

fn via() -> Value {
    object(
        json!({
            "id":positive_id(),
            "position":nm_point(),
            "diameter_nm":integer(1,10_000_000),
            "drill_nm":integer(1,10_000_000),
            "net":pin_ref()
        }),
        &["id", "position", "diameter_nm", "drill_nm", "net"],
    )
}

fn hole() -> Value {
    object(
        json!({
            "id":positive_id(),
            "position":nm_point(),
            "drill_nm":integer(1,10_000_000)
        }),
        &["id", "position", "drill_nm"],
    )
}

fn board() -> Value {
    object(
        json!({
            "width_nm":integer(1_000_000,2_000_000_000),
            "height_nm":integer(1_000_000,2_000_000_000),
            "thickness_nm":integer(100_000,10_000_000),
            "clearance_nm":integer(0,10_000_000),
            "edge_clearance_nm":integer(0,10_000_000),
            "placements":{"type":"object","patternProperties":{"^[0-9]+$":placement()},"additionalProperties":false},
            "tracks":{"type":"object","patternProperties":{"^[0-9]+$":track()},"additionalProperties":false},
            "vias":{"type":"object","patternProperties":{"^[0-9]+$":via()},"additionalProperties":false},
            "holes":{"type":"object","patternProperties":{"^[0-9]+$":hole()},"additionalProperties":false}
        }),
        &[
            "width_nm",
            "height_nm",
            "thickness_nm",
            "clearance_nm",
            "edge_clearance_nm",
            "placements",
            "tracks",
            "vias",
        ],
    )
}

fn model_parameters() -> Value {
    object(
        json!({
            "frequency_hz":{"type":"number","minimum":0.000001,"maximum":1000000000.0},
            "offset_v":{"type":"number"},
            "phase_deg":{"type":"number"},
            "duty_cycle":{"type":"number","exclusiveMinimum":0.0,"exclusiveMaximum":1.0},
            "closed":{"type":"boolean"},
            "wiper":{"type":"number","minimum":0.0,"maximum":1.0},
            "gain":{"type":"number","minimum":1.0,"maximum":1000000.0},
            "rail_low":{"type":"number"},
            "rail_high":{"type":"number"},
            "threshold_v":{"type":"number"},
            "on_resistance":{"type":"number","minimum":0.000001,"maximum":1000000.0},
            "off_resistance":{"type":"number","minimum":1000000.0,"maximum":1000000000000.0},
            "output_resistance":{"type":"number","minimum":0.001,"maximum":1000000.0},
            "saturation_current":{"type":"number","minimum":1e-30,"maximum":1e-6},
            "ideality":{"type":"number","minimum":0.5,"maximum":4.0},
            "mos_threshold_v":{"type":"number","minimum":0.0,"maximum":1000.0},
            "channel_modulation":{"type":"number","minimum":0.0,"maximum":10.0},
            "hysteresis_v":{"type":"number","minimum":0.0,"maximum":1000.0},
            "propagation_delay_s":{"type":"number","minimum":1e-12,"maximum":1.0},
            "initial_state":integer(0,15)
        }),
        &[],
    )
}

fn params_schema(command: &str) -> Option<Value> {
    let id = positive_id;
    let position = point();
    Some(match command {
        "place_device" => object(
            json!({"device":string(256),"position":position}),
            &["device", "position"],
        ),
        "assign_footprint" => object(
            json!({"id":id(),"footprint":string(256)}),
            &["id", "footprint"],
        ),
        "set_board" => object(json!({"board":board()}), &["board"]),
        "place_on_board" => object(
            json!({"id":id(),"placement":placement()}),
            &["id", "placement"],
        ),
        "remove_from_board" | "delete_track" | "delete_via" | "delete_hole"
        | "rotate_component" => object(json!({"id":id()}), &["id"]),
        "add_track" | "update_track" => object(json!({"track":track()}), &["track"]),
        "add_via" | "update_via" => object(json!({"via":via()}), &["via"]),
        "add_hole" | "update_hole" => object(json!({"hole":hole()}), &["hole"]),
        "set_model" => object(
            json!({"id":id(),"parameters":model_parameters()}),
            &["id", "parameters"],
        ),
        "place_component" => object(
            json!({
            "kind":{"type":"string","enum":["Resistor","Capacitor","Inductor","Diode","VoltageSource","CurrentSource","SineVoltage","SquareVoltage","TriangleVoltage","SawVoltage","Switch","Potentiometer","Led","Zener","Npn","Pnp","OpAmp","LogicInput","LogicNot","LogicAnd","LogicOr","LogicXor","LogicNand","LogicNor","Ground","Connector","Nmos","Pmos","Comparator","Schmitt","Timer555","DFlipFlop","Counter4","Counter10","Vcvs","Vccs"]},
                "position":position
            }),
            &["kind", "position"],
        ),
        "move_component" => object(json!({"id":id(),"position":position}), &["id", "position"]),
        "set_component" => object(
            json!({"id":id(),"reference":{"type":"string","minLength":1,"maxLength":64,"pattern":"^[A-Za-z0-9_]+$"},"value":{"type":"number"}}),
            &["id", "reference", "value"],
        ),
        "add_wire" => object(json!({"start":point(),"end":point()}), &["start", "end"]),
        "add_junction" => object(json!({"position":point()}), &["position"]),
        "add_label" => object(
            json!({"position":point(),"name":string(64)}),
            &["position", "name"],
        ),
        "delete" => object(
            json!({"object":object(json!({
            "kind":{"type":"string","enum":["component","wire","junction","label"]},
            "id":id()
        }), &["kind", "id"])}),
            &["object"],
        ),
        "rename_project" => object(json!({"name":string(256)}), &["name"]),
        _ => return None,
    })
}

/// JSON Schema para os parâmetros do comando informado.
pub fn command_schema(command: &str) -> Option<Value> {
    params_schema(command)
}

/// Esquema discriminado para ações em lote, compatível com a entrada tipada de `Action`.
pub fn batch_action_schema() -> Value {
    let variants: Vec<_> = super::COMMANDS
        .iter()
        .filter_map(|command| {
            Some(object(
                json!({
                    "command":{"const":command.id},
                    "params":params_schema(command.id)?
                }),
                &["command", "params"],
            ))
        })
        .collect();
    json!({"oneOf":variants})
}
