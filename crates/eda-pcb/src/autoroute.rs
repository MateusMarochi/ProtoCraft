//! Autorroteador octilinear em duas camadas, independente da interface gráfica.
//!
//! A busca usa uma malha local e esparsa, custos de comprimento/dobra/via,
//! colisão geométrica com índice espacial e limites explícitos de trabalho.

use super::{Airwire, PcbReport, check, placed_pads};
use eda_core::{
    Board, BoardSide, Command, Engine, NM_PER_MM, NmPoint, PadShape, PadTechnology, PinRef,
    Project, Track, Via,
};
use eda_netlist::{NetId, build_netlist};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::time::Instant;

const MIN_GRID_NM: i64 = 50_000;
const MAX_ROUTE_LIMIT: usize = 512;
const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// Limites e custos do autorroteador. Todas as distâncias estão em nanômetros.
#[derive(Debug, Clone)]
pub struct AutorouteConfig {
    pub track_width_nm: i64,
    pub grid_nm: i64,
    pub via_diameter_nm: i64,
    pub via_drill_nm: i64,
    pub bend_penalty_nm: i64,
    pub via_penalty_nm: i64,
    pub congestion_penalty_nm: i64,
    pub max_vias_per_connection: u8,
    pub max_connections: usize,
    pub max_routing_passes: usize,
    pub max_expansions_per_connection: usize,
    pub max_total_expansions: usize,
    pub max_optimization_connections: usize,
    pub prioritize_short_connections: bool,
    pub optimize_routes: bool,
}

impl Default for AutorouteConfig {
    fn default() -> Self {
        Self {
            track_width_nm: 250_000,
            grid_nm: 250_000,
            via_diameter_nm: 600_000,
            via_drill_nm: 300_000,
            bend_penalty_nm: 1_000_000,
            via_penalty_nm: 3 * NM_PER_MM,
            congestion_penalty_nm: 120_000,
            max_vias_per_connection: 4,
            max_connections: 256,
            max_routing_passes: 1,
            max_expansions_per_connection: 128_000,
            max_total_expansions: 600_000,
            max_optimization_connections: 32,
            prioritize_short_connections: false,
            optimize_routes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AutorouteFailure {
    pub net: u32,
    pub start: NmPoint,
    pub end: NmPoint,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AutorouteStats {
    pub routing_passes_attempted: usize,
    pub routing_passes_rejected: usize,
    pub selected_pass: usize,
    pub initial_airwires: usize,
    pub routed_connections: usize,
    pub unrouted_connections: usize,
    pub remaining_airwires: usize,
    pub track_count: usize,
    pub via_count: usize,
    pub total_length_nm: u64,
    pub search_expansions: usize,
    pub optimization_attempts: usize,
    pub optimized_connections: usize,
    pub elapsed_ms: u64,
}

/// Plano já conferido contra o modelo e o DRC; pode ser aplicado como um único comando.
#[derive(Debug, Clone)]
pub struct AutoroutePlan {
    pub commands: Vec<Command>,
    pub stats: AutorouteStats,
    pub failures: Vec<AutorouteFailure>,
}

#[derive(Debug, Clone, Copy)]
struct Endpoint {
    position: NmPoint,
    layers: u8,
    drill_radius_nm: Option<i64>,
}

#[derive(Debug, Clone)]
struct Connection {
    net: NetId,
    anchor: PinRef,
    priority_length_nm: f64,
    track_width_nm: i64,
    via_diameter_nm: i64,
    via_drill_nm: i64,
    start: Endpoint,
    end: Endpoint,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    start: NmPoint,
    end: NmPoint,
    layer: BoardSide,
}

#[derive(Debug, Clone)]
struct RoutedConnection {
    connection: Connection,
    segments: Vec<Segment>,
    vias: Vec<NmPoint>,
    length_nm: f64,
    quality: f64,
}

#[derive(Debug, Clone)]
enum Shape {
    Circle {
        center: NmPoint,
        radius_nm: i64,
    },
    Segment {
        start: NmPoint,
        end: NmPoint,
        radius_nm: i64,
    },
    Polygon(Vec<NmPoint>),
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl Bounds {
    fn from_points(points: &[NmPoint]) -> Option<Self> {
        let first = *points.first()?;
        let mut result = Self {
            min_x: first.x,
            min_y: first.y,
            max_x: first.x,
            max_y: first.y,
        };
        for point in &points[1..] {
            result.min_x = result.min_x.min(point.x);
            result.min_y = result.min_y.min(point.y);
            result.max_x = result.max_x.max(point.x);
            result.max_y = result.max_y.max(point.y);
        }
        Some(result)
    }

    fn expanded(self, amount: i64) -> Self {
        Self {
            min_x: self.min_x.saturating_sub(amount),
            min_y: self.min_y.saturating_sub(amount),
            max_x: self.max_x.saturating_add(amount),
            max_y: self.max_y.saturating_add(amount),
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObstacleKind {
    Copper,
    Via,
    Hole,
    Keepout,
}

#[derive(Debug, Clone)]
struct Obstacle {
    shape: Shape,
    bounds: Bounds,
    layers: u8,
    net: Option<NetId>,
    kind: ObstacleKind,
    blocks_tracks: bool,
    blocks_vias: bool,
}

impl Obstacle {
    fn new(shape: Shape, layers: u8, net: Option<NetId>, kind: ObstacleKind) -> Option<Self> {
        let bounds = match &shape {
            Shape::Circle { center, radius_nm } => Bounds {
                min_x: center.x.saturating_sub(*radius_nm),
                min_y: center.y.saturating_sub(*radius_nm),
                max_x: center.x.saturating_add(*radius_nm),
                max_y: center.y.saturating_add(*radius_nm),
            },
            Shape::Segment {
                start,
                end,
                radius_nm,
            } => Bounds::from_points(&[*start, *end])?.expanded(*radius_nm),
            Shape::Polygon(points) => Bounds::from_points(points)?,
        };
        Some(Self {
            shape,
            bounds,
            layers,
            net,
            kind,
            blocks_tracks: true,
            blocks_vias: true,
        })
    }

    fn keepout(points: Vec<NmPoint>, blocks_tracks: bool, blocks_vias: bool) -> Option<Self> {
        let mut obstacle = Self::new(Shape::Polygon(points), 3, None, ObstacleKind::Keepout)?;
        obstacle.blocks_tracks = blocks_tracks;
        obstacle.blocks_vias = blocks_vias;
        Some(obstacle)
    }
}

/// Índice espacial em grade grossa; objetos extensos ficam em uma lista global limitada.
struct SpatialIndex {
    obstacles: Vec<Obstacle>,
    buckets: HashMap<(i64, i64), Vec<usize>>,
    broad: Vec<usize>,
    seen: Vec<u32>,
    generation: u32,
    bucket_nm: i64,
}

impl SpatialIndex {
    fn new(bucket_nm: i64) -> Self {
        Self {
            obstacles: Vec::new(),
            buckets: HashMap::new(),
            broad: Vec::new(),
            seen: Vec::new(),
            generation: 0,
            bucket_nm: bucket_nm.max(NM_PER_MM),
        }
    }

    fn add(&mut self, obstacle: Obstacle) {
        let index = self.obstacles.len();
        if let Shape::Segment {
            start,
            end,
            radius_nm,
        } = &obstacle.shape
        {
            let x0 = start.x.div_euclid(self.bucket_nm);
            let y0 = start.y.div_euclid(self.bucket_nm);
            let x1 = end.x.div_euclid(self.bucket_nm);
            let y1 = end.y.div_euclid(self.bucket_nm);
            let dx = x1 - x0;
            let dy = y1 - y0;
            let steps = dx.abs().max(dy.abs()).max(1);
            let padding = (*radius_nm)
                .max(0)
                .saturating_add(self.bucket_nm - 1)
                .div_euclid(self.bucket_nm)
                .saturating_add(1);
            self.obstacles.push(obstacle);
            self.seen.push(0);
            if steps > 250_000 || padding > 8 {
                self.broad.push(index);
                return;
            }
            for step_index in 0..=steps {
                let x = x0 + dx * step_index / steps;
                let y = y0 + dy * step_index / steps;
                for bucket_x in x - padding..=x + padding {
                    for bucket_y in y - padding..=y + padding {
                        self.buckets
                            .entry((bucket_x, bucket_y))
                            .or_default()
                            .push(index);
                    }
                }
            }
            return;
        }
        let min_x = obstacle.bounds.min_x.div_euclid(self.bucket_nm);
        let min_y = obstacle.bounds.min_y.div_euclid(self.bucket_nm);
        let max_x = obstacle.bounds.max_x.div_euclid(self.bucket_nm);
        let max_y = obstacle.bounds.max_y.div_euclid(self.bucket_nm);
        let cells = (max_x - min_x + 1) * (max_y - min_y + 1);
        self.obstacles.push(obstacle);
        self.seen.push(0);
        if cells > 256 {
            self.broad.push(index);
            return;
        }
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                self.buckets.entry((x, y)).or_default().push(index);
            }
        }
    }

    fn candidates(&mut self, bounds: Bounds, output: &mut Vec<usize>) {
        output.clear();
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.seen.fill(0);
            self.generation = 1;
        }
        let generation = self.generation;
        for position in 0..self.broad.len() {
            let index = self.broad[position];
            Self::push_candidate(&mut self.seen, index, generation, output);
        }
        let min_x = bounds.min_x.div_euclid(self.bucket_nm);
        let min_y = bounds.min_y.div_euclid(self.bucket_nm);
        let max_x = bounds.max_x.div_euclid(self.bucket_nm);
        let max_y = bounds.max_y.div_euclid(self.bucket_nm);
        let cells = (max_x - min_x + 1) * (max_y - min_y + 1);
        if cells > 4096 {
            for index in 0..self.obstacles.len() {
                if self.obstacles[index].bounds.overlaps(bounds) {
                    Self::push_candidate(&mut self.seen, index, generation, output);
                }
            }
            return;
        }
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if let Some(items) = self.buckets.get(&(x, y)) {
                    for index in items.iter().copied() {
                        Self::push_candidate(&mut self.seen, index, generation, output);
                    }
                }
            }
        }
    }

    fn push_candidate(seen: &mut [u32], index: usize, generation: u32, output: &mut Vec<usize>) {
        if seen[index] != generation {
            seen[index] = generation;
            output.push(index);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SearchState {
    x: i32,
    y: i32,
    layer: u8,
    heading: u8,
    vias: u8,
}

#[derive(Debug, Clone, Copy)]
struct SearchNode {
    state: SearchState,
    cost: f64,
    parent: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct QueueEntry {
    priority: f64,
    cost: f64,
    sequence: u64,
    node: usize,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority.total_cmp(&other.priority) == Ordering::Equal
            && self.sequence == other.sequence
    }
}
impl Eq for QueueEntry {}
impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

struct SearchContext<'a> {
    index: &'a mut SpatialIndex,
    board: &'a Board,
    config: &'a AutorouteConfig,
    net: NetId,
    clearance_nm: i64,
    class_clearances: &'a BTreeMap<NetId, i64>,
    congestion_range_nm: i64,
    candidates: Vec<usize>,
}

impl SearchContext<'_> {
    fn clearance_to(&self, other: Option<NetId>) -> i64 {
        self.clearance_nm
            .max(self.class_clearances.get(&self.net).copied().unwrap_or(0))
            .max(
                other
                    .and_then(|net| self.class_clearances.get(&net).copied())
                    .unwrap_or(0),
            )
    }

    fn segment_cost(&mut self, start: NmPoint, end: NmPoint, layer: u8) -> Option<f64> {
        let route_radius = self.config.track_width_nm / 2;
        if !segment_inside_board(self.board, start, end, route_radius) {
            return None;
        }
        let maximum_clearance = self
            .class_clearances
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .max(self.clearance_nm);
        let query_margin = route_radius
            .saturating_add(maximum_clearance)
            .saturating_add(self.congestion_range_nm);
        let bounds = Bounds::from_points(&[start, end])?.expanded(query_margin);
        self.index.candidates(bounds, &mut self.candidates);
        let mut penalty = 0.0;
        for index in self.candidates.iter().copied() {
            let obstacle = &self.index.obstacles[index];
            if !obstacle.blocks_tracks
                || obstacle.layers & layer == 0
                || obstacle.net == Some(self.net)
            {
                continue;
            }
            let required = route_radius.saturating_add(self.clearance_to(obstacle.net));
            let distance = segment_shape_distance(start, end, &obstacle.shape);
            if distance + 0.5 < required as f64 {
                return None;
            }
            let slack = distance - required as f64;
            if slack < self.congestion_range_nm as f64 {
                let weight = 1.0 - slack.max(0.0) / self.congestion_range_nm as f64;
                penalty += self.config.congestion_penalty_nm as f64 * weight * 0.2;
            }
        }
        Some(point_distance(start, end) + penalty)
    }

    fn via_clear(&mut self, point: NmPoint) -> bool {
        let radius = self.config.via_diameter_nm / 2;
        let edge = self.board.edge_clearance_nm.saturating_add(radius);
        if !self.board.contains_with_clearance(point, edge) {
            return false;
        }
        let maximum_clearance = self
            .class_clearances
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .max(self.clearance_nm);
        let required = radius.saturating_add(maximum_clearance);
        let bounds = Bounds {
            min_x: point.x.saturating_sub(required),
            min_y: point.y.saturating_sub(required),
            max_x: point.x.saturating_add(required),
            max_y: point.y.saturating_add(required),
        };
        self.index.candidates(bounds, &mut self.candidates);
        for index in self.candidates.iter().copied() {
            let obstacle = &self.index.obstacles[index];
            if !obstacle.blocks_vias {
                continue;
            }
            if obstacle.layers != 0
                && obstacle.kind != ObstacleKind::Via
                && obstacle.net == Some(self.net)
            {
                continue;
            }
            let same_net_via = obstacle.kind == ObstacleKind::Via && obstacle.net == Some(self.net);
            let via_clearance = if same_net_via {
                0
            } else {
                self.clearance_to(obstacle.net)
            };
            let minimum = radius.saturating_add(via_clearance);
            let distance = point_shape_distance(point, &obstacle.shape);
            if distance + 0.5 < minimum as f64 {
                return false;
            }
        }
        true
    }
}

/// Calcula conexões, faz o roteamento em uma cópia e rejeita o lote se surgir novo erro físico.
pub fn autoroute(project: &Project, config: &AutorouteConfig) -> Result<AutoroutePlan, String> {
    validate_config(config)?;
    let started = Instant::now();
    let count = check(project)?.airwires.len();
    let unique_orders = if count <= 2 {
        count.max(1)
    } else {
        count.saturating_mul(2)
    };
    let pass_limit = config.max_routing_passes.min(unique_orders);
    let mut remaining = config.max_total_expansions;
    let mut total = 0;
    let mut attempts = 0;
    let mut rejected = 0;
    let mut best: Option<AutoroutePlan> = None;
    for pass in 0..pass_limit {
        if remaining == 0 {
            break;
        }
        let mut attempt = config.clone();
        attempt.max_total_expansions = (remaining / (pass_limit - pass)).max(1);
        let mut used = 0;
        let candidate = autoroute_pass(project, &attempt, pass / 2, pass % 2 == 1, &mut used).map(
            |mut plan| {
                plan.stats.selected_pass = pass + 1;
                plan
            },
        );
        remaining = remaining.saturating_sub(used);
        total += used;
        attempts += 1;
        if !select_candidate(&mut best, candidate, config.via_penalty_nm)? {
            rejected += 1;
        }
        if best
            .as_ref()
            .is_some_and(|p| p.stats.remaining_airwires == 0)
        {
            break;
        }
    }
    let mut plan = best
        .ok_or_else(|| eda_i18n::text("Parâmetros de autorroteamento fora dos limites seguros."))?;
    plan.stats.elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    plan.stats.search_expansions = total;
    plan.stats.routing_passes_attempted = attempts;
    plan.stats.routing_passes_rejected = rejected;
    Ok(plan)
}

// Um candidato recusado pelo DRC não invalida uma tentativa já conferida.
fn select_candidate(
    best: &mut Option<AutoroutePlan>,
    candidate: Result<AutoroutePlan, String>,
    via_penalty_nm: i64,
) -> Result<bool, String> {
    let candidate = match candidate {
        Ok(candidate) => candidate,
        Err(error) if best.is_none() => return Err(error),
        Err(_) => return Ok(false),
    };
    let score = |p: &AutoroutePlan| {
        (
            p.stats.remaining_airwires,
            p.stats
                .total_length_nm
                .saturating_add((p.stats.via_count as u64).saturating_mul(via_penalty_nm as u64)),
        )
    };
    if best
        .as_ref()
        .is_none_or(|current| score(&candidate) < score(current))
    {
        *best = Some(candidate);
    }
    Ok(true)
}

/// Gera fanouts curtos para pads SMD densos; redes de potência usam a regra de escape
/// estreito, enquanto as demais respeitam integralmente largura e via da própria classe.
fn fanout_dense_smd_pads(
    project: &mut Project,
    unresolved: &[Airwire],
    baseline: &PcbReport,
    escape_only: bool,
    standard_only: bool,
) -> Result<Vec<Command>, String> {
    let pads = placed_pads(project)?;
    let netlist = build_netlist(project);
    let board = project
        .board
        .as_deref()
        .cloned()
        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
    let mut dense_components = HashSet::new();
    let mut centers = HashMap::<eda_core::ComponentId, (i128, i128, usize)>::new();
    for pad in pads
        .iter()
        .filter(|pad| pad.pad.technology == PadTechnology::Smd)
    {
        let center = centers.entry(pad.component).or_default();
        center.0 += i128::from(pad.position.x);
        center.1 += i128::from(pad.position.y);
        center.2 += 1;
    }
    for (index, pad) in pads.iter().enumerate() {
        if pad.pad.technology != PadTechnology::Smd {
            continue;
        }
        if pads.iter().skip(index + 1).any(|other| {
            other.component == pad.component
                && other.pad.technology == PadTechnology::Smd
                && point_distance(pad.position, other.position) <= 800_000.0
        }) {
            dense_components.insert(pad.component);
        }
    }
    if dense_components.is_empty() {
        return Ok(Vec::new());
    }
    let mut commands = Vec::new();
    let mut next_track_id = board
        .tracks
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas trilhas."))?;
    let mut next_via_id = board
        .vias
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas vias."))?;

    for pad in pads.iter().filter(|pad| {
        dense_components.contains(&pad.component) && pad.pad.technology == PadTechnology::Smd
    }) {
        let (Some(net), Some(pin)) = (pad.net, pad.pin) else {
            continue;
        };
        // O par de endpoints da airwire representa componentes desconectados
        // da rede; ele pode não incluir o pad QFN específico que precisa de
        // escape. Como o DRC já confirmou que a rede segue aberta, tentamos
        // somente pads densos ainda sem trilha conectada nessa mesma rede.
        let required_for_airwire = unresolved.iter().any(|wire| wire.net == net);
        if !required_for_airwire {
            continue;
        }
        let Some(rules) = super::net_class_for_id(&board, &netlist, net) else {
            continue;
        };
        if escape_only && rules.pad_escape.is_none() {
            continue;
        }
        if standard_only && rules.pad_escape.is_some() {
            continue;
        }
        let (track_width_nm, via_diameter_nm, via_drill_nm, max_escape_length_nm) =
            rules.pad_escape.as_ref().map_or(
                (
                    rules.track_width_nm,
                    rules.via_diameter_nm,
                    rules.via_drill_nm,
                    2 * NM_PER_MM,
                ),
                |escape| {
                    (
                        escape.track_width_nm,
                        escape.via_diameter_nm,
                        escape.via_drill_nm,
                        escape.max_length_nm,
                    )
                },
            );
        let already_connected = board.tracks.values().any(|track| {
            netlist.net_for_pin(track.net) == Some(net)
                && (track.start == pad.position || track.end == pad.position)
        });
        if already_connected {
            continue;
        }
        let Some((sum_x, sum_y, count)) = centers.get(&pad.component).copied() else {
            continue;
        };
        if count == 0 {
            continue;
        }
        let center = NmPoint::new(
            (sum_x / count as i128) as i64,
            (sum_y / count as i128) as i64,
        );
        let dx = (pad.position.x - center.x) as f64;
        let dy = (pad.position.y - center.y) as f64;
        if dx.hypot(dy) <= 1.0 {
            continue;
        }
        let base_angle = dy.atan2(dx);
        let angles = [
            0.0,
            std::f64::consts::FRAC_PI_6,
            -std::f64::consts::FRAC_PI_6,
            std::f64::consts::FRAC_PI_3,
            -std::f64::consts::FRAC_PI_3,
            std::f64::consts::FRAC_PI_2,
            -std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
        ];
        let lengths = [650_000_i64, 850_000, 1_100_000, 1_400_000];
        let side = if pad.layers & 1 != 0 {
            BoardSide::Top
        } else if pad.layers & 2 != 0 {
            BoardSide::Bottom
        } else {
            continue;
        };
        let mut accepted = false;
        'candidate: for length in lengths {
            if length > max_escape_length_nm {
                continue;
            }
            for offset in angles {
                let angle = base_angle + offset;
                let via_position = NmPoint::new(
                    pad.position.x + (angle.cos() * length as f64).round() as i64,
                    pad.position.y + (angle.sin() * length as f64).round() as i64,
                );
                let track = Track {
                    id: next_track_id,
                    start: pad.position,
                    end: via_position,
                    width_nm: track_width_nm,
                    layer: side,
                    net: pin,
                };
                let via = Via {
                    id: next_via_id,
                    position: via_position,
                    diameter_nm: via_diameter_nm,
                    drill_nm: via_drill_nm,
                    net: pin,
                };
                {
                    let board = project
                        .board
                        .as_mut()
                        .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
                    board.tracks.insert(track.id, track.clone());
                    board.vias.insert(via.id, via.clone());
                }
                let after = check(project)?;
                let legal = after.errors() <= baseline.errors()
                    && after.airwires.len() <= baseline.airwires.len()
                    && validate_physical_delta(baseline, &after).is_ok();
                if legal {
                    commands.push(Command::AddTrack { track });
                    commands.push(Command::AddVia { via });
                    next_track_id = next_track_id
                        .checked_add(1)
                        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas trilhas."))?;
                    next_via_id = next_via_id
                        .checked_add(1)
                        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas vias."))?;
                    accepted = true;
                    break 'candidate;
                }
                let board = project
                    .board
                    .as_mut()
                    .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
                board.tracks.remove(&track.id);
                board.vias.remove(&via.id);
            }
        }
        if !accepted {
            // Se não houver corredor de escape legal, a busca normal ainda pode
            // rotear o pad; não introduzimos geometria que viole o DRC.
            continue;
        }
    }
    Ok(commands)
}

fn has_dense_unescaped_airwire(project: &Project, unresolved: &[Airwire]) -> Result<bool, String> {
    let pads = placed_pads(project)?;
    let board = project.board.as_deref().ok_or("O projeto não tem PCB.")?;
    let netlist = build_netlist(project);
    for pad in pads
        .iter()
        .filter(|pad| pad.pad.technology == PadTechnology::Smd)
    {
        let Some(net) = pad.net else {
            continue;
        };
        let touched = unresolved.iter().any(|wire| wire.net == net);
        if !touched {
            continue;
        }
        let dense = pads.iter().any(|other| {
            other.component == pad.component
                && other.pad.technology == PadTechnology::Smd
                && other.position != pad.position
                && point_distance(pad.position, other.position) <= 800_000.0
        });
        if !dense {
            continue;
        }
        if super::net_class_for_id(board, &netlist, net)
            .is_some_and(|rules| rules.pad_escape.is_none())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn connections_for_airwires(
    airwires: &[Airwire],
    pads: &[super::PlacedPad],
    board: &Board,
    netlist: &eda_netlist::Netlist,
    config: &AutorouteConfig,
) -> (Vec<Connection>, Vec<AutorouteFailure>) {
    let anchors = net_anchors(pads, board, netlist);
    let mut connections = Vec::with_capacity(airwires.len());
    let mut failures = Vec::new();
    for airwire in airwires {
        let Some(anchor) = anchors.get(&airwire.net).copied() else {
            failures.push(failure(
                airwire,
                "A rede não tem pino de referência na PCB.",
            ));
            continue;
        };
        let start = prefer_connected_via(
            pads,
            board,
            netlist,
            airwire.net,
            airwire.start,
            airwire.end,
        );
        let end = prefer_connected_via(pads, board, netlist, airwire.net, airwire.end, start);
        let start_layers = endpoint_layers(pads, board, netlist, airwire.net, start);
        let end_layers = endpoint_layers(pads, board, netlist, airwire.net, end);
        if start_layers == 0 || end_layers == 0 {
            failures.push(failure(
                airwire,
                "Não foi possível identificar a camada do cobre em uma ponta.",
            ));
            continue;
        }
        let rules = super::net_class_for_id(board, netlist, airwire.net);
        connections.push(Connection {
            net: airwire.net,
            anchor,
            priority_length_nm: point_distance(airwire.start, airwire.end),
            track_width_nm: rules.map_or(config.track_width_nm, |rules| rules.track_width_nm),
            via_diameter_nm: rules.map_or(config.via_diameter_nm, |rules| rules.via_diameter_nm),
            via_drill_nm: rules.map_or(config.via_drill_nm, |rules| rules.via_drill_nm),
            start: Endpoint {
                position: start,
                layers: start_layers,
                drill_radius_nm: endpoint_drill_radius(pads, board, netlist, airwire.net, start),
            },
            end: Endpoint {
                position: end,
                layers: end_layers,
                drill_radius_nm: endpoint_drill_radius(pads, board, netlist, airwire.net, end),
            },
        });
    }
    (connections, failures)
}

fn route_geometry_commands(
    routes: &[RoutedConnection],
    board: &Board,
) -> Result<Vec<Command>, String> {
    let mut next_track_id = board
        .tracks
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas trilhas."))?;
    let mut next_via_id = board
        .vias
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas vias."))?;
    let mut commands = Vec::new();
    for route in routes {
        for segment in &route.segments {
            if segment.start == segment.end {
                continue;
            }
            commands.push(Command::AddTrack {
                track: Track {
                    id: next_track_id,
                    start: segment.start,
                    end: segment.end,
                    width_nm: route.connection.track_width_nm,
                    layer: segment.layer,
                    net: route.connection.anchor,
                },
            });
            next_track_id = next_track_id
                .checked_add(1)
                .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas trilhas."))?;
        }
        for position in &route.vias {
            commands.push(Command::AddVia {
                via: Via {
                    id: next_via_id,
                    position: *position,
                    diameter_nm: route.connection.via_diameter_nm,
                    drill_nm: route.connection.via_drill_nm,
                    net: route.connection.anchor,
                },
            });
            next_via_id = next_via_id
                .checked_add(1)
                .ok_or_else(|| eda_i18n::text("Não há IDs livres para novas vias."))?;
        }
    }
    Ok(commands)
}

fn autoroute_pass(
    project: &Project,
    config: &AutorouteConfig,
    rotation: usize,
    reverse: bool,
    consumed_expansions: &mut usize,
) -> Result<AutoroutePlan, String> {
    validate_config(config)?;
    let started = Instant::now();
    let source_before = check(project)?;
    let mut routing_project = project.clone();
    let mut initial_fanout_commands = fanout_dense_smd_pads(
        &mut routing_project,
        &source_before.airwires,
        &source_before,
        true,
        false,
    )?;
    let board = routing_project
        .board
        .as_deref()
        .ok_or("O projeto não tem PCB.")?;
    let before = check(&routing_project)?;
    let pads = placed_pads(&routing_project)?;
    let netlist = build_netlist(&routing_project);
    let reserve_fallback = if has_dense_unescaped_airwire(&routing_project, &before.airwires)? {
        config.max_total_expansions / 5
    } else {
        0
    };
    let primary_budget = config.max_total_expansions - reserve_fallback;
    let class_clearances: BTreeMap<_, _> = netlist
        .nets
        .iter()
        .filter_map(|net| {
            super::net_class_for_id(board, &netlist, net.id)
                .map(|rules| (net.id, rules.clearance_nm))
        })
        .collect();
    let (mut connections, mut failures) =
        connections_for_airwires(&before.airwires, &pads, board, &netlist, config);
    connections.sort_by(|a, b| {
        if config.prioritize_short_connections {
            a.priority_length_nm.total_cmp(&b.priority_length_nm)
        } else {
            b.priority_length_nm.total_cmp(&a.priority_length_nm)
        }
    });

    if reverse {
        connections.reverse();
    }
    if !connections.is_empty() {
        let amount = rotation % connections.len();
        connections.rotate_left(amount);
    }
    let mut index = build_obstacle_index(&routing_project, &pads, &netlist, config)?;
    let mut routes = Vec::new();
    let mut expansions = 0usize;
    for (position, connection) in connections.iter().enumerate() {
        if position >= config.max_connections {
            failures.push(connection_failure(
                connection,
                "O limite de conexões por execução foi atingido.",
            ));
            continue;
        }
        if expansions >= primary_budget {
            failures.push(connection_failure(
                connection,
                "O orçamento global de busca foi atingido.",
            ));
            continue;
        }
        let budget = config
            .max_expansions_per_connection
            .min(primary_budget - expansions);
        let route_config = config_for_connection(config, connection);
        let result = search_connection(
            board,
            &mut index,
            connection,
            &route_config,
            &class_clearances,
            budget,
        );
        expansions = expansions.saturating_add(result.expansions);
        *consumed_expansions = expansions;
        match result.route {
            Some(route) => {
                add_route_obstacles(&mut index, &route, connection.net, &route_config);
                routes.push(route);
            }
            None => failures.push(connection_failure(
                connection,
                if result.expansions >= budget {
                    "Busca limitada pelo orçamento de expansões; tente grade maior ou mais orçamento."
                } else {
                    "Não foi encontrado caminho legal nas duas camadas."
                },
            )),
        }
    }

    let mut optimization_attempts = 0usize;
    let mut optimized_connections = 0usize;
    if config.optimize_routes && !routes.is_empty() {
        let optimize_count = routes.len().min(config.max_optimization_connections);
        let optimization_budget = (config.max_total_expansions / 5).max(1);
        let per_route_budget = (optimization_budget / optimize_count.max(1)).max(1);
        for route_index in (0..optimize_count).rev() {
            if expansions >= primary_budget {
                break;
            }
            let mut local_index = build_obstacle_index(&routing_project, &pads, &netlist, config)?;
            for (other_index, other) in routes.iter().enumerate() {
                if other_index != route_index {
                    let other_config = config_for_connection(config, &other.connection);
                    add_route_obstacles(
                        &mut local_index,
                        other,
                        other.connection.net,
                        &other_config,
                    );
                }
            }
            optimization_attempts += 1;
            let remaining = per_route_budget.min(primary_budget - expansions);
            let route_config = config_for_connection(config, &routes[route_index].connection);
            let candidate = search_connection(
                board,
                &mut local_index,
                &routes[route_index].connection,
                &route_config,
                &class_clearances,
                remaining,
            );
            expansions = expansions.saturating_add(candidate.expansions);
            *consumed_expansions = expansions;
            if let Some(candidate_route) = candidate.route
                && candidate_route.quality + 1.0 < routes[route_index].quality
            {
                routes[route_index] = candidate_route;
                optimized_connections += 1;
            }
        }
    }

    let primary_route_count = routes.len();
    let primary_route_commands = route_geometry_commands(&routes, board)?;
    let mut fallback_fanout_commands = Vec::new();
    let mut retry_route_commands = Vec::new();
    if reserve_fallback > 0 && !failures.is_empty() && expansions < config.max_total_expansions {
        let mut retry_engine =
            Engine::new(routing_project.clone()).map_err(|error| error.to_string())?;
        if !primary_route_commands.is_empty() {
            retry_engine
                .execute(Command::Batch(primary_route_commands.clone()))
                .map_err(|error| format!("O lote inicial de rotas foi recusado: {error}"))?;
        }
        let mut retry_project = retry_engine.project().clone();
        let retry_baseline = check(&retry_project)?;
        fallback_fanout_commands = fanout_dense_smd_pads(
            &mut retry_project,
            &retry_baseline.airwires,
            &retry_baseline,
            false,
            false,
        )?;
        let retry_report = check(&retry_project)?;
        let retry_pads = placed_pads(&retry_project)?;
        let retry_board = retry_project
            .board
            .as_deref()
            .ok_or_else(|| eda_i18n::text("O projeto não tem PCB."))?;
        let mut retry_index = build_obstacle_index(&retry_project, &retry_pads, &netlist, config)?;
        let (mut retry_connections, mut retry_failures) = connections_for_airwires(
            &retry_report.airwires,
            &retry_pads,
            retry_board,
            &netlist,
            config,
        );
        retry_connections.sort_by(|a, b| {
            if config.prioritize_short_connections {
                a.priority_length_nm.total_cmp(&b.priority_length_nm)
            } else {
                b.priority_length_nm.total_cmp(&a.priority_length_nm)
            }
        });
        if reverse {
            retry_connections.reverse();
        }
        if !retry_connections.is_empty() {
            let amount = rotation % retry_connections.len();
            retry_connections.rotate_left(amount);
        }
        for (position, connection) in retry_connections.iter().enumerate() {
            if position >= config.max_connections || expansions >= config.max_total_expansions {
                retry_failures.push(connection_failure(
                    connection,
                    "O orçamento global de busca foi atingido.",
                ));
                continue;
            }
            let budget = config
                .max_expansions_per_connection
                .min(config.max_total_expansions - expansions);
            let route_config = config_for_connection(config, connection);
            let result = search_connection(
                retry_board,
                &mut retry_index,
                connection,
                &route_config,
                &class_clearances,
                budget,
            );
            expansions = expansions.saturating_add(result.expansions);
            *consumed_expansions = expansions;
            match result.route {
                Some(route) => {
                    add_route_obstacles(&mut retry_index, &route, connection.net, &route_config);
                    routes.push(route);
                }
                None => retry_failures.push(connection_failure(
                    connection,
                    if result.expansions >= budget {
                        "Busca limitada pelo orçamento de expansões; tente grade maior ou mais orçamento."
                    } else {
                        "Não foi encontrado caminho legal nas duas camadas."
                    },
                )),
            }
        }
        failures = retry_failures;
        retry_route_commands =
            route_geometry_commands(&routes[primary_route_count..], retry_board)?;
    }

    let mut commands = Vec::with_capacity(
        initial_fanout_commands.len()
            + primary_route_commands.len()
            + fallback_fanout_commands.len()
            + retry_route_commands.len(),
    );
    commands.append(&mut initial_fanout_commands);
    commands.extend(primary_route_commands);
    commands.extend(fallback_fanout_commands);
    commands.extend(retry_route_commands);

    let mut stats = AutorouteStats {
        initial_airwires: source_before.airwires.len(),
        routed_connections: routes.len(),
        unrouted_connections: failures.len(),
        track_count: commands
            .iter()
            .filter(|command| matches!(command, Command::AddTrack { .. }))
            .count(),
        via_count: commands
            .iter()
            .filter(|command| matches!(command, Command::AddVia { .. }))
            .count(),
        total_length_nm: routes
            .iter()
            .map(|route| route.length_nm.max(0.0).round() as u64)
            .sum(),
        search_expansions: expansions,
        optimization_attempts,
        optimized_connections,
        ..AutorouteStats::default()
    };
    if !commands.is_empty() {
        let mut candidate_engine =
            Engine::new(project.clone()).map_err(|error| error.to_string())?;
        candidate_engine
            .execute(Command::Batch(commands.clone()))
            .map_err(|error| format!("O modelo recusou o plano de roteamento: {error}"))?;
        let after = check(candidate_engine.project())?;
        validate_physical_delta(&source_before, &after)?;
        let expected_remaining = source_before.airwires.len().saturating_sub(routes.len());
        if after.airwires.len() > expected_remaining {
            return Err(format!(
                "Autorroteamento cancelado: o DRC ainda detectou {remaining} conexões pendentes de {planned} rotas planejadas.",
                remaining = after.airwires.len(),
                planned = routes.len()
            ));
        }
        stats.remaining_airwires = after.airwires.len();
    } else {
        stats.remaining_airwires = source_before.airwires.len();
    }
    stats.elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    Ok(AutoroutePlan {
        commands,
        stats,
        failures,
    })
}

fn validate_config(config: &AutorouteConfig) -> Result<(), String> {
    if !(1_000..=10 * NM_PER_MM).contains(&config.track_width_nm)
        || !(MIN_GRID_NM..=10 * NM_PER_MM).contains(&config.grid_nm)
        || !(100_000..=20 * NM_PER_MM).contains(&config.via_diameter_nm)
        || !(1_000..config.via_diameter_nm).contains(&config.via_drill_nm)
        || config.max_vias_per_connection > 16
        || !(1..=8).contains(&config.max_routing_passes)
        || config.max_connections == 0
        || config.max_connections > MAX_ROUTE_LIMIT
        || config.max_expansions_per_connection == 0
        || config.max_expansions_per_connection > 1_000_000
        || config.max_total_expansions == 0
        || config.max_total_expansions > 5_000_000
        || config.max_optimization_connections > MAX_ROUTE_LIMIT
        || config.bend_penalty_nm < 0
        || config.via_penalty_nm < 0
        || config.congestion_penalty_nm < 0
        || config.bend_penalty_nm > 100 * NM_PER_MM
        || config.via_penalty_nm > 100 * NM_PER_MM
        || config.congestion_penalty_nm > 10 * NM_PER_MM
    {
        return Err("Parâmetros de autorroteamento fora dos limites seguros.".into());
    }
    if config.via_diameter_nm - config.via_drill_nm < 200_000 {
        return Err("A via precisa manter anel de cobre mínimo de 0,1 mm.".into());
    }
    Ok(())
}

fn failure(wire: &Airwire, reason: &str) -> AutorouteFailure {
    AutorouteFailure {
        net: wire.net.0,
        start: wire.start,
        end: wire.end,
        reason: reason.into(),
    }
}

fn connection_failure(connection: &Connection, reason: &str) -> AutorouteFailure {
    AutorouteFailure {
        net: connection.net.0,
        start: connection.start.position,
        end: connection.end.position,
        reason: reason.into(),
    }
}

fn layer_bit(layer: BoardSide) -> u8 {
    match layer {
        BoardSide::Top => 1,
        BoardSide::Bottom => 2,
    }
}

fn side_for_bit(layer: u8) -> BoardSide {
    if layer == 1 {
        BoardSide::Top
    } else {
        BoardSide::Bottom
    }
}

fn net_anchors(
    pads: &[super::PlacedPad],
    board: &Board,
    netlist: &eda_netlist::Netlist,
) -> HashMap<NetId, PinRef> {
    let mut anchors = HashMap::new();
    for pad in pads {
        if let (Some(net), Some(pin)) = (pad.net, pad.pin) {
            anchors.entry(net).or_insert(pin);
        }
    }
    for track in board.tracks.values() {
        if let Some(net) = netlist.net_for_pin(track.net) {
            anchors.entry(net).or_insert(track.net);
        }
    }
    for via in board.vias.values() {
        if let Some(net) = netlist.net_for_pin(via.net) {
            anchors.entry(net).or_insert(via.net);
        }
    }
    anchors
}

fn prefer_connected_via(
    pads: &[super::PlacedPad],
    board: &Board,
    netlist: &eda_netlist::Netlist,
    net: NetId,
    point: NmPoint,
    destination: NmPoint,
) -> NmPoint {
    let same_pad = |endpoint| {
        pads.iter().any(|pad| {
            pad.net == Some(net)
                && pad.position == endpoint
                && (pad.position == point
                    || pad
                        .polygons
                        .iter()
                        .any(|polygon| point_in_polygon(point, polygon))
                    || (pad.pad.shape == PadShape::Circle
                        && point_distance(pad.position, point) <= pad.pad.size.x as f64 / 2.0))
        })
    };
    // Uma ponta visual do airwire pode cair no canto de um pad com fanout.
    // Reutilizar sua via já conectada permite iniciar na outra face sem procurar
    // novamente um escape na grade, preservando a ilha elétrica de origem.
    board
        .vias
        .values()
        .filter(|via| netlist.net_for_pin(via.net) == Some(net))
        .filter(|via| {
            via.position == point
                || board.tracks.values().any(|track| {
                    netlist.net_for_pin(track.net) == Some(net)
                        && ((track.start == via.position
                            && (track.end == point || same_pad(track.end)))
                            || (track.end == via.position
                                && (track.start == point || same_pad(track.start))))
                })
        })
        .min_by(|a, b| {
            point_distance(a.position, destination)
                .total_cmp(&point_distance(b.position, destination))
        })
        .map_or(point, |via| via.position)
}

fn endpoint_layers(
    pads: &[super::PlacedPad],
    board: &Board,
    netlist: &eda_netlist::Netlist,
    net: NetId,
    point: NmPoint,
) -> u8 {
    let mut layers = 0;
    for pad in pads.iter().filter(|pad| pad.net == Some(net)) {
        let at_pad = pad.position == point
            || pad
                .polygons
                .iter()
                .any(|polygon| point_in_polygon(point, polygon));
        if at_pad {
            layers |= pad.layers;
        }
    }
    for track in board.tracks.values() {
        if netlist.net_for_pin(track.net) == Some(net)
            && (track.start == point || track.end == point)
        {
            layers |= layer_bit(track.layer);
        }
    }
    for via in board.vias.values() {
        if netlist.net_for_pin(via.net) == Some(net) && via.position == point {
            layers |= 3;
        }
    }
    // Imported and hand-edited boards can have a track start at a point inside a pad,
    // so exact geometry above is sufficient; this final check catches circular pads.
    if layers == 0 {
        for pad in pads.iter().filter(|pad| pad.net == Some(net)) {
            if pad.pad.shape == PadShape::Circle
                && point_distance(pad.position, point) <= pad.pad.size.x as f64 / 2.0 + 1.0
            {
                layers |= pad.layers;
            }
        }
    }
    layers & 3
}

fn endpoint_drill_radius(
    pads: &[super::PlacedPad],
    board: &Board,
    netlist: &eda_netlist::Netlist,
    net: NetId,
    point: NmPoint,
) -> Option<i64> {
    pads.iter()
        .filter(|pad| {
            pad.net == Some(net)
                && pad.position == point
                && pad.pad.technology == PadTechnology::ThroughHole
        })
        .filter_map(|pad| pad.pad.drill)
        .map(|drill| drill.x.max(drill.y) / 2)
        .chain(board.vias.values().filter_map(|via| {
            (via.position == point && netlist.net_for_pin(via.net) == Some(net))
                .then_some(via.drill_nm / 2)
        }))
        .max()
}

fn build_obstacle_index(
    project: &Project,
    pads: &[super::PlacedPad],
    netlist: &eda_netlist::Netlist,
    config: &AutorouteConfig,
) -> Result<SpatialIndex, String> {
    let board = project.board.as_deref().ok_or("O projeto não tem PCB.")?;
    let mut index = SpatialIndex::new((config.grid_nm.saturating_mul(16)).max(4 * NM_PER_MM));
    for pad in pads {
        if pad.pad.technology == PadTechnology::NonPlated {
            if let Some(size) = pad.pad.drill
                && let Some(obstacle) = Obstacle::new(
                    Shape::Circle {
                        center: pad.position,
                        radius_nm: size.x.max(size.y) / 2,
                    },
                    3,
                    None,
                    ObstacleKind::Hole,
                )
            {
                index.add(obstacle);
            }
            continue;
        }
        if pad.layers == 0 {
            continue;
        }
        let net = pad.net;
        if pad.pad.shape == PadShape::Circle && pad.pad.size.x == pad.pad.size.y {
            if let Some(obstacle) = Obstacle::new(
                Shape::Circle {
                    center: pad.position,
                    radius_nm: pad.pad.size.x / 2,
                },
                pad.layers,
                net,
                ObstacleKind::Copper,
            ) {
                index.add(obstacle);
            }
            continue;
        }
        for polygon in &pad.polygons {
            if polygon.len() >= 3
                && let Some(obstacle) = Obstacle::new(
                    Shape::Polygon(polygon.clone()),
                    pad.layers,
                    net,
                    ObstacleKind::Copper,
                )
            {
                index.add(obstacle);
            }
        }
    }
    for track in board.tracks.values() {
        if let Some(obstacle) = Obstacle::new(
            Shape::Segment {
                start: track.start,
                end: track.end,
                radius_nm: track.width_nm / 2,
            },
            layer_bit(track.layer),
            netlist.net_for_pin(track.net),
            ObstacleKind::Copper,
        ) {
            index.add(obstacle);
        }
    }
    for via in board.vias.values() {
        if let Some(obstacle) = Obstacle::new(
            Shape::Circle {
                center: via.position,
                radius_nm: via.diameter_nm / 2,
            },
            3,
            netlist.net_for_pin(via.net),
            ObstacleKind::Via,
        ) {
            index.add(obstacle);
        }
    }
    for hole in board.holes.values() {
        if let Some(obstacle) = Obstacle::new(
            Shape::Circle {
                center: hole.position,
                radius_nm: hole.drill_nm / 2,
            },
            3,
            None,
            ObstacleKind::Hole,
        ) {
            index.add(obstacle);
        }
    }
    for (component_id, placement) in &board.placements {
        let Some(footprint) = project
            .components
            .get(component_id)
            .and_then(|item| item.footprint.as_ref())
        else {
            continue;
        };
        for keepout in &footprint.keepouts {
            if !keepout.tracks && !keepout.vias && !keepout.copper {
                continue;
            }
            let polygon: Vec<_> = keepout
                .polygon
                .iter()
                .copied()
                .map(|point| placement.transform(point))
                .collect();
            if polygon.len() >= 3
                && let Some(obstacle) = Obstacle::keepout(
                    polygon,
                    keepout.tracks || keepout.copper,
                    keepout.vias || keepout.copper,
                )
            {
                index.add(obstacle);
            }
        }
    }
    Ok(index)
}

fn add_route_obstacles(
    index: &mut SpatialIndex,
    route: &RoutedConnection,
    net: NetId,
    config: &AutorouteConfig,
) {
    for segment in &route.segments {
        if let Some(obstacle) = Obstacle::new(
            Shape::Segment {
                start: segment.start,
                end: segment.end,
                radius_nm: config.track_width_nm / 2,
            },
            layer_bit(segment.layer),
            Some(net),
            ObstacleKind::Copper,
        ) {
            index.add(obstacle);
        }
    }
    for position in &route.vias {
        if let Some(obstacle) = Obstacle::new(
            Shape::Circle {
                center: *position,
                radius_nm: config.via_diameter_nm / 2,
            },
            3,
            Some(net),
            ObstacleKind::Via,
        ) {
            index.add(obstacle);
        }
    }
}

struct SearchResult {
    route: Option<RoutedConnection>,
    expansions: usize,
}

fn config_for_connection(base: &AutorouteConfig, connection: &Connection) -> AutorouteConfig {
    let mut config = base.clone();
    config.track_width_nm = connection.track_width_nm;
    config.via_diameter_nm = connection.via_diameter_nm;
    config.via_drill_nm = connection.via_drill_nm;
    config
}

fn search_connection(
    board: &Board,
    index: &mut SpatialIndex,
    connection: &Connection,
    config: &AutorouteConfig,
    class_clearances: &BTreeMap<NetId, i64>,
    budget: usize,
) -> SearchResult {
    if budget == 0 {
        return SearchResult {
            route: None,
            expansions: 0,
        };
    }
    let mut expansions = 0usize;
    for layer in [1, 2] {
        if connection.start.layers & layer != 0 && connection.end.layers & layer != 0 {
            let mut context = SearchContext {
                index: &mut *index,
                board,
                config,
                net: connection.net,
                clearance_nm: board.clearance_nm,
                class_clearances,
                congestion_range_nm: (config.grid_nm * 4).max(NM_PER_MM),
                candidates: Vec::with_capacity(64),
            };
            if octilinear_or_short(
                connection.start.position,
                connection.end.position,
                config.grid_nm.max(MIN_GRID_NM).saturating_mul(2),
            ) && context
                .segment_cost(connection.start.position, connection.end.position, layer)
                .is_some()
            {
                let Some(route) = make_route(
                    connection.clone(),
                    vec![
                        (connection.start.position, side_for_bit(layer)),
                        (connection.end.position, side_for_bit(layer)),
                    ],
                    config,
                    &mut context,
                ) else {
                    continue;
                };
                return SearchResult {
                    route: Some(route),
                    expansions,
                };
            }
        }
    }

    let base_step = config.grid_nm.max(MIN_GRID_NM);
    let base_margin = (base_step.saturating_mul(20)).max(4 * NM_PER_MM);
    let attempts = [
        (base_step, base_margin),
        (base_step, base_margin.saturating_mul(2)),
        (
            (base_step / 2).max(MIN_GRID_NM),
            base_margin.saturating_mul(3),
        ),
        (
            (base_step / 2).max(MIN_GRID_NM),
            base_margin.saturating_mul(6),
        ),
    ];
    for (step, margin) in attempts {
        if expansions >= budget {
            break;
        }
        let remaining = budget - expansions;
        // Each resolution gets a bounded share so one broad maze cannot starve retries.
        let attempt_budget = remaining.min((budget / 2).max(1));
        let result = astar_attempt(
            board,
            index,
            connection,
            config,
            class_clearances,
            SearchAttempt {
                step,
                margin,
                budget: attempt_budget,
            },
        );
        expansions = expansions.saturating_add(result.1);
        if let Some(route) = result.0 {
            return SearchResult {
                route: Some(route),
                expansions,
            };
        }
    }
    SearchResult {
        route: None,
        expansions,
    }
}

struct SearchAttempt {
    step: i64,
    margin: i64,
    budget: usize,
}

fn astar_attempt(
    board: &Board,
    index: &mut SpatialIndex,
    connection: &Connection,
    config: &AutorouteConfig,
    class_clearances: &BTreeMap<NetId, i64>,
    attempt: SearchAttempt,
) -> (Option<RoutedConnection>, usize) {
    let SearchAttempt {
        step,
        margin,
        budget,
    } = attempt;
    let start = connection.start.position;
    let end = connection.end.position;
    let min_x = start.x.min(end.x).saturating_sub(margin).max(0);
    let min_y = start.y.min(end.y).saturating_sub(margin).max(0);
    let max_x = start
        .x
        .max(end.x)
        .saturating_add(margin)
        .min(board.width_nm);
    let max_y = start
        .y
        .max(end.y)
        .saturating_add(margin)
        .min(board.height_nm);
    let nx = ((max_x - min_x).max(0) as f64 / step as f64).ceil() as i32;
    let ny = ((max_y - min_y).max(0) as f64 / step as f64).ceil() as i32;
    let max_nodes = budget.saturating_mul(8).clamp(64, 2_000_000);
    let mut nodes: Vec<SearchNode> = Vec::with_capacity(budget.min(8192));
    let mut by_state: HashMap<SearchState, usize> = HashMap::with_capacity(budget.min(8192));
    let mut open = BinaryHeap::with_capacity(budget.min(8192));
    let mut sequence = 0u64;
    let mut context = SearchContext {
        index,
        board,
        config,
        net: connection.net,
        clearance_nm: board.clearance_nm,
        class_clearances,
        congestion_range_nm: (step.saturating_mul(4)).max(NM_PER_MM),
        candidates: Vec::with_capacity(64),
    };

    let mut best_goal: Option<(f64, usize)> = None;
    for source_layer in [1, 2] {
        if connection.start.layers & source_layer == 0 {
            continue;
        }
        let base_x = ((start.x - min_x) as f64 / step as f64).round() as i32;
        let base_y = ((start.y - min_y) as f64 / step as f64).round() as i32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let x = base_x.saturating_add(dx);
                let y = base_y.saturating_add(dy);
                if x < 0 || y < 0 || x > nx || y > ny {
                    continue;
                }
                let point = grid_point(min_x, min_y, x, y, step);
                if !octilinear_or_short(start, point, step.saturating_mul(2)) {
                    continue;
                }
                let Some(connector_cost) = context.segment_cost(start, point, source_layer) else {
                    continue;
                };
                let state = SearchState {
                    x,
                    y,
                    layer: source_layer,
                    heading: 8,
                    vias: 0,
                };
                insert_state(
                    state,
                    connector_cost,
                    None,
                    end,
                    min_x,
                    min_y,
                    step,
                    &mut nodes,
                    &mut by_state,
                    &mut open,
                    &mut sequence,
                    max_nodes,
                );
            }
        }
    }

    let mut expanded = 0usize;
    while let Some(entry) = open.pop() {
        let Some(node) = nodes.get(entry.node).copied() else {
            continue;
        };
        if node.cost.total_cmp(&entry.cost) != Ordering::Equal {
            continue;
        }
        if let Some((goal_cost, _)) = best_goal
            && entry.priority >= goal_cost
        {
            break;
        }
        expanded += 1;
        if expanded > budget || nodes.len() >= max_nodes {
            break;
        }
        let current = grid_point(min_x, min_y, node.state.x, node.state.y, step);
        let current_layer = node.state.layer;
        if connection.end.layers & current_layer != 0
            && octilinear_or_short(current, end, step.saturating_mul(2))
            && let Some(edge_cost) = context.segment_cost(current, end, current_layer)
        {
            let heading = direction_of(current, end);
            let bend = if node.state.heading < 8 && heading < 8 && node.state.heading != heading {
                config.bend_penalty_nm as f64
            } else {
                0.0
            };
            let total = node.cost + edge_cost + bend;
            if best_goal.is_none_or(|(best, _)| total < best) {
                best_goal = Some((total, entry.node));
            }
        }
        for (heading, (dx, dy)) in DIRECTIONS.iter().copied().enumerate() {
            let x = node.state.x.saturating_add(dx);
            let y = node.state.y.saturating_add(dy);
            if x < 0 || y < 0 || x > nx || y > ny {
                continue;
            }
            let next = grid_point(min_x, min_y, x, y, step);
            let Some(edge_cost) = context.segment_cost(current, next, current_layer) else {
                continue;
            };
            let bend_cost = if node.state.heading < 8 && node.state.heading as usize != heading {
                config.bend_penalty_nm as f64
            } else {
                0.0
            };
            let state = SearchState {
                x,
                y,
                layer: current_layer,
                heading: heading as u8,
                vias: node.state.vias,
            };
            insert_state(
                state,
                node.cost + edge_cost + bend_cost,
                Some(entry.node),
                end,
                min_x,
                min_y,
                step,
                &mut nodes,
                &mut by_state,
                &mut open,
                &mut sequence,
                max_nodes,
            );
        }
        if node.state.vias < config.max_vias_per_connection {
            let other_layer = if current_layer == 1 { 2 } else { 1 };
            if context.via_clear(current) {
                let state = SearchState {
                    x: node.state.x,
                    y: node.state.y,
                    layer: other_layer,
                    heading: 8,
                    vias: node.state.vias + 1,
                };
                insert_state(
                    state,
                    node.cost + config.via_penalty_nm as f64,
                    Some(entry.node),
                    end,
                    min_x,
                    min_y,
                    step,
                    &mut nodes,
                    &mut by_state,
                    &mut open,
                    &mut sequence,
                    max_nodes,
                );
            }
        }
    }

    let Some((_, goal_node)) = best_goal else {
        return (None, expanded.min(budget));
    };
    let mut state_path = Vec::new();
    let mut cursor = Some(goal_node);
    while let Some(index) = cursor {
        let Some(node) = nodes.get(index) else {
            return (None, expanded);
        };
        state_path.push(node.state);
        cursor = node.parent;
    }
    state_path.reverse();
    let mut waypoints = Vec::with_capacity(state_path.len() + 2);
    waypoints.push((
        start,
        side_for_bit(state_path.first().map_or(1, |state| state.layer)),
    ));
    for state in state_path {
        let item = (
            grid_point(min_x, min_y, state.x, state.y, step),
            side_for_bit(state.layer),
        );
        if waypoints.last().copied() != Some(item) {
            waypoints.push(item);
        }
    }
    let final_layer = waypoints.last().map_or(BoardSide::Top, |item| item.1);
    if connection.end.layers & layer_bit(final_layer) == 0 {
        return (None, expanded);
    }
    if waypoints.last().is_some_and(|item| item.0 != end) {
        waypoints.push((end, final_layer));
    }
    (
        make_route(connection.clone(), waypoints, config, &mut context),
        expanded.min(budget),
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_state(
    state: SearchState,
    cost: f64,
    parent: Option<usize>,
    goal: NmPoint,
    origin_x: i64,
    origin_y: i64,
    step: i64,
    nodes: &mut Vec<SearchNode>,
    by_state: &mut HashMap<SearchState, usize>,
    open: &mut BinaryHeap<QueueEntry>,
    sequence: &mut u64,
    max_nodes: usize,
) {
    if !cost.is_finite() || nodes.len() >= max_nodes {
        return;
    }
    if let Some(index) = by_state.get(&state).copied() {
        if nodes[index].cost <= cost {
            return;
        }
        nodes[index].cost = cost;
        nodes[index].parent = parent;
        let point = grid_point(origin_x, origin_y, state.x, state.y, step);
        let priority = cost + point_distance(point, goal);
        open.push(QueueEntry {
            priority,
            cost,
            sequence: *sequence,
            node: index,
        });
    } else {
        let index = nodes.len();
        nodes.push(SearchNode {
            state,
            cost,
            parent,
        });
        by_state.insert(state, index);
        let point = grid_point(origin_x, origin_y, state.x, state.y, step);
        open.push(QueueEntry {
            priority: cost + point_distance(point, goal),
            cost,
            sequence: *sequence,
            node: index,
        });
    }
    *sequence = (*sequence).saturating_add(1);
}

fn grid_point(origin_x: i64, origin_y: i64, x: i32, y: i32, step: i64) -> NmPoint {
    NmPoint::new(
        origin_x.saturating_add(i64::from(x).saturating_mul(step)),
        origin_y.saturating_add(i64::from(y).saturating_mul(step)),
    )
}

fn make_route(
    connection: Connection,
    mut waypoints: Vec<(NmPoint, BoardSide)>,
    config: &AutorouteConfig,
    context: &mut SearchContext<'_>,
) -> Option<RoutedConnection> {
    if !trim_drilled_pad_approaches(&connection, &mut waypoints, config, context) {
        return None;
    }
    let mut segments: Vec<Segment> = Vec::new();
    let mut vias = Vec::new();
    for pair in waypoints.windows(2) {
        let (start, start_layer) = pair[0];
        let (end, end_layer) = pair[1];
        if start == end {
            if start_layer != end_layer {
                vias.push(start);
            }
            continue;
        }
        if start_layer != end_layer {
            continue;
        }
        if let Some(last) = segments.last_mut()
            && last.layer == start_layer
            && last.end == start
            && collinear_forward(last.start, last.end, end)
        {
            last.end = end;
        } else if start != end {
            segments.push(Segment {
                start,
                end,
                layer: start_layer,
            });
        }
    }
    vias.dedup();
    let length_nm: f64 = segments
        .iter()
        .map(|segment| point_distance(segment.start, segment.end))
        .sum();
    let mut bends = 0usize;
    for pair in segments.windows(2) {
        if pair[0].layer == pair[1].layer
            && pair[0].end == pair[1].start
            && direction_of(pair[0].start, pair[0].end) != direction_of(pair[1].start, pair[1].end)
        {
            bends += 1;
        }
    }
    let quality = length_nm
        + vias.len() as f64 * config.via_penalty_nm as f64
        + bends as f64 * config.bend_penalty_nm as f64;
    Some(RoutedConnection {
        connection,
        segments,
        vias,
        length_nm,
        quality,
    })
}

fn trim_drilled_pad_approaches(
    connection: &Connection,
    waypoints: &mut Vec<(NmPoint, BoardSide)>,
    config: &AutorouteConfig,
    context: &mut SearchContext<'_>,
) -> bool {
    if waypoints.len() < 2 {
        return false;
    }

    let start_radius = connection.start.drill_radius_nm.map(|drill_radius| {
        drill_radius
            .saturating_add(config.track_width_nm / 2)
            .saturating_add(50_000)
    });
    if let Some(radius_nm) = start_radius {
        let Some(join_index) =
            waypoints
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(index, (join, layer))| {
                    let distance = point_distance(connection.start.position, *join);
                    (distance > radius_nm as f64
                        && connection.start.layers & layer_bit(*layer) != 0
                        && context
                            .segment_cost(connection.start.position, *join, layer_bit(*layer))
                            .is_some())
                    .then_some(index)
                })
        else {
            return false;
        };
        let (join, layer) = waypoints[join_index];
        let entry = radial_entry_point(connection.start.position, join, radius_nm);
        let mut trimmed = Vec::with_capacity(waypoints.len() + 1);
        trimmed.push((entry, layer));
        trimmed.extend_from_slice(&waypoints[join_index..]);
        *waypoints = trimmed;
    }

    if waypoints.len() < 2 {
        return false;
    }
    let end_radius = connection.end.drill_radius_nm.map(|drill_radius| {
        drill_radius
            .saturating_add(config.track_width_nm / 2)
            .saturating_add(50_000)
    });
    if let Some(radius_nm) = end_radius {
        let Some(join_index) =
            waypoints
                .iter()
                .enumerate()
                .rev()
                .skip(1)
                .find_map(|(index, (join, layer))| {
                    let distance = point_distance(connection.end.position, *join);
                    (distance > radius_nm as f64
                        && connection.end.layers & layer_bit(*layer) != 0
                        && context
                            .segment_cost(connection.end.position, *join, layer_bit(*layer))
                            .is_some())
                    .then_some(index)
                })
        else {
            return false;
        };
        let (join, layer) = waypoints[join_index];
        let entry = radial_entry_point(connection.end.position, join, radius_nm);
        waypoints.truncate(join_index + 1);
        waypoints.push((entry, layer));
    }
    true
}

fn radial_entry_point(center: NmPoint, toward: NmPoint, radius_nm: i64) -> NmPoint {
    let dx = (toward.x - center.x) as f64;
    let dy = (toward.y - center.y) as f64;
    let distance = dx.hypot(dy);
    if distance <= 0.0 {
        return center;
    }
    NmPoint::new(
        center.x + (dx * radius_nm as f64 / distance).round() as i64,
        center.y + (dy * radius_nm as f64 / distance).round() as i64,
    )
}

fn collinear_forward(a: NmPoint, b: NmPoint, c: NmPoint) -> bool {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let bc_x = c.x - b.x;
    let bc_y = c.y - b.y;
    i128::from(ab_x) * i128::from(bc_y) == i128::from(ab_y) * i128::from(bc_x)
        && ab_x.signum() == bc_x.signum()
        && ab_y.signum() == bc_y.signum()
}

fn direction_of(start: NmPoint, end: NmPoint) -> u8 {
    let dx = (end.x - start.x).signum() as i32;
    let dy = (end.y - start.y).signum() as i32;
    DIRECTIONS
        .iter()
        .position(|candidate| *candidate == (dx, dy))
        .map_or(8, |index| index as u8)
}

fn octilinear_or_short(start: NmPoint, end: NmPoint, max_stub_nm: i64) -> bool {
    let dx = start.x.abs_diff(end.x);
    let dy = start.y.abs_diff(end.y);
    dx == 0 || dy == 0 || dx == dy || point_distance(start, end) <= max_stub_nm as f64
}

fn segment_inside_board(board: &Board, start: NmPoint, end: NmPoint, radius_nm: i64) -> bool {
    let margin = board.edge_clearance_nm.saturating_add(radius_nm);
    [start, end]
        .iter()
        .all(|point| board.contains_with_clearance(*point, margin))
}

fn point_distance(a: NmPoint, b: NmPoint) -> f64 {
    ((a.x - b.x) as f64).hypot((a.y - b.y) as f64)
}

fn point_segment_distance(point: NmPoint, start: NmPoint, end: NmPoint) -> f64 {
    let dx = (end.x - start.x) as f64;
    let dy = (end.y - start.y) as f64;
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((point.x - start.x) as f64 * dx + (point.y - start.y) as f64 * dy) / length_squared)
            .clamp(0.0, 1.0)
    };
    ((point.x - start.x) as f64 - t * dx).hypot((point.y - start.y) as f64 - t * dy)
}

fn orientation(a: NmPoint, b: NmPoint, c: NmPoint) -> i128 {
    i128::from(b.x - a.x) * i128::from(c.y - a.y) - i128::from(b.y - a.y) * i128::from(c.x - a.x)
}

fn on_segment(point: NmPoint, start: NmPoint, end: NmPoint) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
        && orientation(start, end, point) == 0
}

fn segments_intersect(a: NmPoint, b: NmPoint, c: NmPoint, d: NmPoint) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);
    (o1.signum() != o2.signum() && o3.signum() != o4.signum())
        || (o1 == 0 && on_segment(c, a, b))
        || (o2 == 0 && on_segment(d, a, b))
        || (o3 == 0 && on_segment(a, c, d))
        || (o4 == 0 && on_segment(b, c, d))
}

fn segment_segment_distance(a: NmPoint, b: NmPoint, c: NmPoint, d: NmPoint) -> f64 {
    if segments_intersect(a, b, c, d) {
        0.0
    } else {
        point_segment_distance(a, c, d)
            .min(point_segment_distance(b, c, d))
            .min(point_segment_distance(c, a, b))
            .min(point_segment_distance(d, a, b))
    }
}

fn point_in_polygon(point: NmPoint, polygon: &[NmPoint]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    for (start, end) in polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
    {
        if point_segment_distance(point, start, end) <= 0.5 {
            return true;
        }
        if (start.y > point.y) != (end.y > point.y)
            && (point.x as f64)
                < (end.x - start.x) as f64 * (point.y - start.y) as f64 / (end.y - start.y) as f64
                    + start.x as f64
        {
            inside = !inside;
        }
    }
    inside
}

fn segment_shape_distance(start: NmPoint, end: NmPoint, shape: &Shape) -> f64 {
    match shape {
        Shape::Circle { center, radius_nm } => {
            point_segment_distance(*center, start, end) - *radius_nm as f64
        }
        Shape::Segment {
            start: other_start,
            end: other_end,
            radius_nm,
        } => segment_segment_distance(start, end, *other_start, *other_end) - *radius_nm as f64,
        Shape::Polygon(polygon) => {
            if point_in_polygon(start, polygon) || point_in_polygon(end, polygon) {
                return 0.0;
            }
            polygon
                .iter()
                .copied()
                .zip(polygon.iter().copied().cycle().skip(1))
                .take(polygon.len())
                .map(|(a, b)| segment_segment_distance(start, end, a, b))
                .fold(f64::INFINITY, f64::min)
        }
    }
}

fn point_shape_distance(point: NmPoint, shape: &Shape) -> f64 {
    match shape {
        Shape::Circle { center, radius_nm } => point_distance(point, *center) - *radius_nm as f64,
        Shape::Segment {
            start,
            end,
            radius_nm,
        } => point_segment_distance(point, *start, *end) - *radius_nm as f64,
        Shape::Polygon(polygon) => {
            if point_in_polygon(point, polygon) {
                0.0
            } else {
                polygon
                    .iter()
                    .copied()
                    .zip(polygon.iter().copied().cycle().skip(1))
                    .take(polygon.len())
                    .map(|(start, end)| point_segment_distance(point, start, end))
                    .fold(f64::INFINITY, f64::min)
            }
        }
    }
}

fn validate_physical_delta(
    before: &super::PcbReport,
    after: &super::PcbReport,
) -> Result<(), String> {
    let is_physical = |diagnostic: &super::PcbDiagnostic| {
        diagnostic.severity == "error"
            && matches!(
                diagnostic.code.as_str(),
                "COPPER_SHORT"
                    | "CLEARANCE"
                    | "BOARD_EDGE"
                    | "ANNULAR_RING"
                    | "KEEPOUT"
                    | "HOLE_EDGE"
                    | "HOLE_COPPER"
                    | "HOLE_CLEARANCE"
                    | "HOLE_KEEPOUT"
            )
    };
    let signature = |diagnostic: &super::PcbDiagnostic| {
        format!(
            "{}|{}|{:?}",
            diagnostic.code, diagnostic.message, diagnostic.position
        )
    };
    let mut baseline = BTreeMap::<String, usize>::new();
    for diagnostic in before
        .diagnostics
        .iter()
        .filter(|diagnostic| is_physical(diagnostic))
    {
        *baseline.entry(signature(diagnostic)).or_default() += 1;
    }
    for diagnostic in after
        .diagnostics
        .iter()
        .filter(|diagnostic| is_physical(diagnostic))
    {
        let key = signature(diagnostic);
        if let Some(count) = baseline.get_mut(&key)
            && *count > 0
        {
            *count -= 1;
        } else {
            return Err(format!(
                "Autorroteamento cancelado: o DRC detectou nova violação {code}: {message}",
                code = diagnostic.code,
                message = diagnostic.message
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod pass_selection_tests {
    use super::*;
    fn plan(remaining: usize, selected_pass: usize) -> AutoroutePlan {
        AutoroutePlan {
            commands: vec![],
            failures: vec![],
            stats: AutorouteStats {
                remaining_airwires: remaining,
                selected_pass,
                ..Default::default()
            },
        }
    }
    #[test]
    fn invalid_later_pass_preserves_valid_result_and_first_error_is_reported() {
        let mut best = None;
        assert_eq!(
            select_candidate(&mut best, Err("invalid".into()), 0),
            Err("invalid".into())
        );
        assert!(select_candidate(&mut best, Ok(plan(2, 1)), 0).unwrap());
        assert!(!select_candidate(&mut best, Err("drc".into()), 0).unwrap());
        assert_eq!(best.as_ref().unwrap().stats.selected_pass, 1);
        select_candidate(&mut best, Ok(plan(3, 2)), 0).unwrap();
        assert_eq!(best.as_ref().unwrap().stats.selected_pass, 1);
        select_candidate(&mut best, Ok(plan(1, 3)), 0).unwrap();
        assert_eq!(best.as_ref().unwrap().stats.selected_pass, 3);
    }
}

#[cfg(test)]
mod dense_fanout_tests {
    use super::*;

    #[test]
    fn control_fanout_uses_full_class_width_and_passes_drc() {
        let mut project = eda_library::buck_pi4_project().expect("projeto buck válido");
        let board = project.board.as_mut().expect("PCB do buck");
        board.tracks.clear();
        board.vias.clear();
        let baseline = check(&project).expect("DRC antes do fanout");
        let commands =
            fanout_dense_smd_pads(&mut project, &baseline.airwires, &baseline, false, true)
                .expect("fanout de controle");

        assert!(commands.iter().any(|command| matches!(
            command,
            Command::AddTrack { track }
                if track.net.component == eda_core::ComponentId(2)
                    && track.net.pin == 1
                    && track.width_nm == 250_000
        )));
        let after = check(&project).expect("DRC após o fanout");
        assert!(
            validate_physical_delta(&baseline, &after).is_ok(),
            "fanout de controle não pode introduzir violação geométrica"
        );
    }
}
