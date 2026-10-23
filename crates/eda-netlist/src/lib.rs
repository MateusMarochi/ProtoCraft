//! Conectividade autoritativa: cruzamentos sem junção não conectam.
use eda_core::{ComponentKind, ObjectId, PinRef, Point, Project};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NetId(pub u32);
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Net {
    pub id: NetId,
    pub name: String,
    pub pins: Vec<PinRef>,
    pub points: Vec<Point>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Netlist {
    pub nets: Vec<Net>,
    pub pin_nets: BTreeMap<PinRef, NetId>,
}
impl Netlist {
    pub fn net_for_pin(&self, pin: PinRef) -> Option<NetId> {
        self.pin_nets.get(&pin).copied()
    }
    pub fn is_grounded(&self) -> bool {
        self.nets.iter().any(|n| n.id == NetId(0))
    }
    pub fn net_for_point(&self, point: Point) -> Option<NetId> {
        self.nets
            .iter()
            .find(|n| n.points.binary_search(&point).is_ok())
            .map(|n| n.id)
    }
}
struct UnionFind {
    parents: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parents: (0..n).collect(),
        }
    }
    fn root(&mut self, mut i: usize) -> usize {
        while self.parents[i] != i {
            self.parents[i] = self.parents[self.parents[i]];
            i = self.parents[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let a = self.root(a);
        let b = self.root(b);
        if a != b {
            self.parents[a.max(b)] = a.min(b);
        }
    }
}
pub fn build_netlist(project: &Project) -> Netlist {
    let mut all = BTreeSet::new();
    for c in project.components.values() {
        for pin in c
            .pins()
            .into_iter()
            .filter(|p| c.is_electrical_pin(p.reference.pin))
        {
            all.insert(pin.position);
        }
    }
    for w in project.wires.values() {
        all.insert(w.start);
        all.insert(w.end);
    }
    for j in project.junctions.values() {
        all.insert(j.position);
    }
    for l in project.labels.values() {
        all.insert(l.position);
    }
    let points: Vec<Point> = all.into_iter().collect();
    let indexes: BTreeMap<Point, usize> = points
        .iter()
        .copied()
        .enumerate()
        .map(|(i, p)| (p, i))
        .collect();
    let mut horizontal: BTreeMap<i32, BTreeMap<i32, usize>> = BTreeMap::new();
    let mut vertical: BTreeMap<i32, BTreeMap<i32, usize>> = BTreeMap::new();
    for (i, p) in points.iter().enumerate() {
        horizontal.entry(p.y).or_default().insert(p.x, i);
        vertical.entry(p.x).or_default().insert(p.y, i);
    }
    let mut uf = UnionFind::new(points.len());
    for w in project.wires.values() {
        let Some(&start) = indexes.get(&w.start) else {
            continue;
        };
        if w.start.y == w.end.y {
            if let Some(line) = horizontal.get(&w.start.y) {
                for (_, i) in line.range(w.start.x.min(w.end.x)..=w.start.x.max(w.end.x)) {
                    uf.join(start, *i);
                }
            }
        } else if w.start.x == w.end.x
            && let Some(line) = vertical.get(&w.start.x)
        {
            for (_, i) in line.range(w.start.y.min(w.end.y)..=w.start.y.max(w.end.y)) {
                uf.join(start, *i);
            }
        }
    }
    let mut first_ground = None;
    for c in project
        .components
        .values()
        .filter(|c| c.kind == ComponentKind::Ground)
    {
        if let Some(&i) = indexes.get(&c.position) {
            if let Some(g) = first_ground {
                uf.join(g, i);
            } else {
                first_ground = Some(i);
            }
        }
    }
    let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
    for l in project.labels.values() {
        if let Some(&i) = indexes.get(&l.position) {
            if let Some(&other) = labels.get(l.name.as_str()) {
                uf.join(i, other);
            } else {
                labels.insert(&l.name, i);
            }
        }
    }
    let ground_root = first_ground.map(|i| uf.root(i));
    let mut groups: BTreeMap<usize, Vec<Point>> = BTreeMap::new();
    for (i, p) in points.iter().enumerate() {
        groups.entry(uf.root(i)).or_default().push(*p);
    }
    let mut names_by_root: BTreeMap<usize, BTreeSet<&str>> = BTreeMap::new();
    for label in project.labels.values() {
        if let Some(&index) = indexes.get(&label.position) {
            names_by_root
                .entry(uf.root(index))
                .or_default()
                .insert(&label.name);
        }
    }
    let mut roots = BTreeMap::new();
    let mut nets = Vec::new();
    let mut next = 1;
    for (root, points) in groups {
        let id = if Some(root) == ground_root {
            NetId(0)
        } else {
            let id = NetId(next);
            next += 1;
            id
        };
        roots.insert(root, id);
        let name = if id == NetId(0) {
            "0".into()
        } else if let Some(name) = names_by_root.get(&root).and_then(|names| names.first()) {
            name.to_string()
        } else {
            format!("N{}", id.0)
        };
        nets.push(Net {
            id,
            name,
            pins: Vec::new(),
            points,
        });
    }
    nets.sort_by_key(|n| n.id);
    let mut pin_nets = BTreeMap::new();
    for c in project.components.values() {
        for pin in c
            .pins()
            .into_iter()
            .filter(|p| c.is_electrical_pin(p.reference.pin))
        {
            if let Some(&i) = indexes.get(&pin.position)
                && let Some(&id) = roots.get(&uf.root(i))
            {
                pin_nets.insert(pin.reference, id);
            }
        }
    }
    let mut pins_by_net: BTreeMap<NetId, Vec<PinRef>> = BTreeMap::new();
    for (pin, net) in &pin_nets {
        pins_by_net.entry(*net).or_default().push(*pin);
    }
    for net in &mut nets {
        net.pins = pins_by_net.remove(&net.id).unwrap_or_default();
    }
    Netlist { nets, pin_nets }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub objects: Vec<ObjectId>,
}
pub fn erc(project: &Project, netlist: &Netlist) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let nets_by_id: BTreeMap<NetId, &Net> = netlist.nets.iter().map(|net| (net.id, net)).collect();
    let point_nets: BTreeMap<Point, NetId> = netlist
        .nets
        .iter()
        .flat_map(|net| net.points.iter().map(|point| (*point, net.id)))
        .collect();
    let mut names_by_net: BTreeMap<NetId, BTreeSet<&str>> = BTreeMap::new();
    for label in project.labels.values() {
        if let Some(net) = point_nets.get(&label.position) {
            names_by_net.entry(*net).or_default().insert(&label.name);
        }
    }
    let mut push = |severity, code: &str, message: String, objects| {
        out.push(Diagnostic {
            severity,
            code: code.into(),
            message,
            objects,
        })
    };
    if let Err(e) = project.validate() {
        push(
            Severity::Error,
            "INVALID_PROJECT",
            e.to_string(),
            Vec::new(),
        );
    }
    if project
        .import_info
        .as_ref()
        .is_some_and(|info| !info.electrical_complete)
    {
        push(Severity::Error,"INCOMPLETE_IMPORT","A importação não recuperou toda a conectividade. Simulação bloqueada; confira o relatório de importação.".into(),Vec::new());
    }
    for component in project.components.values().filter(|component| {
        matches!(
            component.kind,
            ComponentKind::Imported | ComponentKind::Device
        )
    }) {
        push(
            Severity::Warning,
            "UNSUPPORTED_MODEL",
            format!(
                "{reference} foi preservado com seus pinos, mas seu modelo ainda não é suportado.",
                reference = component.reference
            ),
            vec![ObjectId::Component(component.id)],
        );
    }
    // Uma importação parcial não permite distinguir ilhas reais de conexões omitidas.
    if project
        .import_info
        .as_ref()
        .is_some_and(|info| !info.electrical_complete)
    {
        return out;
    }
    if !netlist.is_grounded() {
        push(
            Severity::Error,
            "NO_GROUND",
            "Adicione um terra (GND) ao circuito para definir o nó de referência.".into(),
            Vec::new(),
        );
    }
    if !project
        .components
        .values()
        .any(|c| c.kind.is_voltage_source() || c.kind == ComponentKind::CurrentSource)
    {
        push(
            Severity::Warning,
            "NO_SOURCE",
            "O circuito não contém fonte independente de tensão ou corrente.".into(),
            Vec::new(),
        );
    }
    let mut adjacency: BTreeMap<NetId, BTreeSet<NetId>> = BTreeMap::new();
    for c in project.components.values() {
        let pins = c.pins();
        if pins.len() > 2 && c.kind != ComponentKind::Device {
            let nets: Vec<_> = pins
                .iter()
                .filter_map(|p| netlist.net_for_pin(p.reference))
                .collect();
            for &a in &nets {
                for &b in &nets {
                    adjacency.entry(a).or_default().insert(b);
                }
            }
        }
        if pins.len() == 2
            && let (Some(a), Some(b)) = (
                netlist.net_for_pin(pins[0].reference),
                netlist.net_for_pin(pins[1].reference),
            )
        {
            adjacency.entry(a).or_default().insert(b);
            adjacency.entry(b).or_default().insert(a);
            if a == b {
                push(
                    if c.kind.is_voltage_source() && c.value != 0.0 {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                    "SHORTED_COMPONENT",
                    format!(
                        "{reference} está com os dois pinos na mesma rede.",
                        reference = c.reference
                    ),
                    vec![ObjectId::Component(c.id)],
                );
            }
        }
    }
    let mut reachable = BTreeSet::new();
    let mut stack = vec![NetId(0)];
    while let Some(id) = stack.pop() {
        if reachable.insert(id)
            && let Some(neighbors) = adjacency.get(&id)
        {
            stack.extend(neighbors.iter().copied());
        }
    }
    for c in project
        .components
        .values()
        .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector))
    {
        for p in c.pins() {
            if !c.is_electrical_pin(p.reference.pin) {
                if project.labels.values().any(|l| l.position == p.position)
                    || project.wires.values().any(|w| {
                        if w.start.x == w.end.x {
                            p.position.x == w.start.x
                                && p.position.y >= w.start.y.min(w.end.y)
                                && p.position.y <= w.start.y.max(w.end.y)
                        } else {
                            p.position.y == w.start.y
                                && p.position.x >= w.start.x.min(w.end.x)
                                && p.position.x <= w.start.x.max(w.end.x)
                        }
                    })
                {
                    push(
                        Severity::Error,
                        "NC_CONNECTED",
                        format!(
                            "{reference}: pino {pin} é NC e não deve receber conexão.",
                            reference = c.reference,
                            pin = c.pin_number(p.reference.pin)
                        ),
                        vec![ObjectId::Component(c.id)],
                    );
                }
                continue;
            }
            if let Some(id) = netlist.net_for_pin(p.reference)
                && let Some(net) = nets_by_id.get(&id)
                && net.pins.len() < 2
            {
                push(
                    Severity::Warning,
                    "UNCONNECTED_PIN",
                    format!(
                        "{reference}: pino {pin} sem outro terminal conectado.",
                        reference = c.reference,
                        pin = c.pin_number(p.reference.pin)
                    ),
                    vec![ObjectId::Component(c.id)],
                );
            }
        }
        if netlist.is_grounded()
            && c.pins().iter().all(|p| {
                netlist
                    .net_for_pin(p.reference)
                    .is_none_or(|id| !reachable.contains(&id))
            })
        {
            push(
                Severity::Error,
                "FLOATING_CIRCUIT",
                format!(
                    "{reference} pertence a uma ilha sem ligação ao terra.",
                    reference = c.reference
                ),
                vec![ObjectId::Component(c.id)],
            );
        }
    }
    for net in &netlist.nets {
        let names = names_by_net.remove(&net.id).unwrap_or_default();
        if names.len() > 1 {
            push(
                Severity::Warning,
                "NET_ALIASES",
                format!(
                    "A rede {net} reúne nomes diferentes: {names}.",
                    net = net.name,
                    names = names.into_iter().collect::<Vec<_>>().join(", ")
                ),
                Vec::new(),
            );
        }
        if net.pins.is_empty() {
            push(
                Severity::Warning,
                "EMPTY_NET",
                format!(
                    "A rede {net} contém fios ou rótulos sem componentes.",
                    net = net.name
                ),
                Vec::new(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::{Command, Engine};
    fn engine() -> Engine {
        Engine::new(Project::empty("teste")).unwrap()
    }
    fn wires(e: &mut Engine) {
        e.execute(Command::Batch(vec![
            Command::AddWire {
                start: Point::new(-5, 0),
                end: Point::new(5, 0),
            },
            Command::AddWire {
                start: Point::new(0, -5),
                end: Point::new(0, 5),
            },
        ]))
        .unwrap();
    }
    #[test]
    fn crossing_is_not_junction() {
        let mut e = engine();
        wires(&mut e);
        assert_eq!(build_netlist(e.project()).nets.len(), 2);
        e.execute(Command::AddJunction {
            position: Point::new(0, 0),
        })
        .unwrap();
        assert_eq!(build_netlist(e.project()).nets.len(), 1);
    }
    #[test]
    fn t_junction_connects() {
        let mut e = engine();
        e.execute(Command::Batch(vec![
            Command::AddWire {
                start: Point::new(-5, 0),
                end: Point::new(5, 0),
            },
            Command::AddWire {
                start: Point::new(0, -5),
                end: Point::new(0, 0),
            },
        ]))
        .unwrap();
        assert_eq!(build_netlist(e.project()).nets.len(), 1);
    }
    #[test]
    fn labels_are_case_sensitive() {
        let mut e = engine();
        for (name, x) in [("VOUT", 0), ("VOUT", 10), ("vout", 20)] {
            e.execute(Command::AddLabel {
                position: Point::new(x, 0),
                name: name.into(),
            })
            .unwrap();
        }
        assert_eq!(build_netlist(e.project()).nets.len(), 2);
    }
    #[test]
    fn examples_are_deterministic_and_grounded() {
        for p in [
            eda_core::examples::voltage_divider(),
            eda_core::examples::rc_filter(),
            eda_core::examples::diode_limiter(),
        ] {
            p.validate().unwrap();
            let n = build_netlist(&p);
            assert_eq!(n.nets.len(), 3);
            assert!(n.is_grounded());
            assert_eq!(n, build_netlist(&p));
            assert!(erc(&p, &n).is_empty());
        }
    }
    #[test]
    fn ground_symbols_merge() {
        let mut e = engine();
        for x in [0, 10] {
            e.execute(Command::PlaceComponent {
                kind: ComponentKind::Ground,
                position: Point::new(x, 0),
            })
            .unwrap();
        }
        let n = build_netlist(e.project());
        assert_eq!(n.nets.len(), 1);
        assert_eq!(n.nets[0].id, NetId(0));
    }
    #[test]
    fn moving_and_rotating_changes_real_connectivity() {
        let mut e = Engine::new(eda_core::examples::voltage_divider()).unwrap();
        let id = e
            .project()
            .components
            .values()
            .find(|c| c.reference == "R1")
            .unwrap()
            .id;
        let original = build_netlist(e.project());
        e.execute(Command::MoveComponent {
            id,
            position: Point::new(0, 30),
        })
        .unwrap();
        assert_eq!(build_netlist(e.project()).nets.len(), 5);
        e.undo();
        assert_eq!(build_netlist(e.project()), original);
        e.execute(Command::RotateComponent { id }).unwrap();
        assert_eq!(build_netlist(e.project()).nets.len(), 5);
    }
    #[test]
    fn orphan_is_error() {
        let mut p = eda_core::examples::voltage_divider();
        let id = p
            .components
            .values()
            .find(|c| c.kind == ComponentKind::Ground)
            .unwrap()
            .id;
        p.components.remove(&id);
        assert!(
            erc(&p, &build_netlist(&p))
                .iter()
                .any(|d| d.code == "NO_GROUND")
        );
    }
    #[test]
    fn partial_import_reports_incomplete_without_claiming_missing_circuit_errors() {
        let mut project = eda_core::Project::empty("Proteus parcial");
        project.import_info = Some(eda_core::ImportedProjectInfo {
            source_format: "Proteus ISIS".into(),
            electrical_complete: false,
            notes: vec!["Objetos especiais ainda não mapeados.".into()],
        });
        let diagnostics = erc(&project, &build_netlist(&project));
        assert!(diagnostics.iter().any(|d| d.code == "INCOMPLETE_IMPORT"));
        assert!(!diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "NO_GROUND" | "NO_SOURCE" | "FLOATING_CIRCUIT"
            )
        }));
    }
    #[test]
    fn overlapping_wires_merge() {
        let mut e = engine();
        e.execute(Command::Batch(vec![
            Command::AddWire {
                start: Point::new(0, 0),
                end: Point::new(10, 0),
            },
            Command::AddWire {
                start: Point::new(5, 0),
                end: Point::new(15, 0),
            },
        ]))
        .unwrap();
        assert_eq!(build_netlist(e.project()).nets.len(), 1);
    }
}
