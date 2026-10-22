use super::*;
use std::fmt::Write;
fn q(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}
fn layer(name: &str, side: BoardSide) -> String {
    if side == BoardSide::Bottom {
        if let Some(rest) = name.strip_prefix("F.") {
            return format!("B.{rest}");
        }
        if let Some(rest) = name.strip_prefix("B.") {
            return format!("F.{rest}");
        }
    }
    name.into()
}
fn local(p: NmPoint, placement: &Placement) -> NmPoint {
    let t = placement.transform(p);
    NmPoint::new(t.x - placement.position.x, t.y - placement.position.y)
}
/// Formato aberto de intercâmbio. Não exige KiCad instalado para gerar o arquivo.
pub fn export_kicad(project: &Project) -> Result<String, String> {
    project.validate().map_err(|e| e.to_string())?;
    let board = project.board.as_ref().ok_or("O projeto não tem PCB.")?;
    let netlist = build_netlist(project);
    let mut out = String::from("(kicad_pcb (version 20240108) (generator protocraft)\n");
    writeln!(
        out,
        "(general (thickness {:.6}))\n(paper \"A4\")",
        mm(board.thickness_nm)
    )
    .map_err(|e| e.to_string())?;
    out.push_str("(layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (34 \"B.Paste\" user) (35 \"F.Paste\" user) (36 \"B.SilkS\" user) (37 \"F.SilkS\" user) (38 \"B.Mask\" user) (39 \"F.Mask\" user) (40 \"Dwgs.User\" user) (41 \"Cmts.User\" user) (42 \"Eco1.User\" user) (43 \"Eco2.User\" user) (44 \"Edge.Cuts\" user) (46 \"B.CrtYd\" user) (47 \"F.CrtYd\" user) (48 \"B.Fab\" user) (49 \"F.Fab\" user))\n(setup (pad_to_mask_clearance 0))\n(net 0 \"\")\n");
    for net in &netlist.nets {
        writeln!(out, "(net {} {})", net.id.0 + 1, q(&net.name)).map_err(|e| e.to_string())?;
    }
    for (id, placement) in &board.placements {
        let c = &project.components[id];
        let fp = c.footprint.as_ref().ok_or("Footprint ausente")?;
        writeln!(
            out,
            "(footprint {} (layer {}) (at {:.6} {:.6})",
            q(&fp.id),
            q(&layer("F.Cu", placement.side)),
            mm(placement.position.x),
            mm(placement.position.y)
        )
        .map_err(|e| e.to_string())?;
        let at = local(NmPoint::new(0, fp.body_bounds.min.y - 1_000_000), placement);
        writeln!(out,"(fp_text reference {} (at {:.6} {:.6}) (layer {}) (effects (font (size 1 1) (thickness 0.15))))",q(&c.reference),mm(at.x),mm(at.y),q(&layer("F.SilkS",placement.side))).map_err(|e|e.to_string())?;
        for line in &fp.lines {
            let a = local(line.start, placement);
            let b = local(line.end, placement);
            writeln!(out,"(fp_line (start {:.6} {:.6}) (end {:.6} {:.6}) (stroke (width {:.6}) (type solid)) (layer {}))",mm(a.x),mm(a.y),mm(b.x),mm(b.y),mm(line.width_nm),q(&layer(&line.layer,placement.side))).map_err(|e|e.to_string())?;
        }
        let mut mapping = BTreeMap::new();
        for pin in c.pins() {
            for number in c.pads_for_pin(pin.reference.pin) {
                mapping.insert(number, pin.reference);
            }
        }
        for pad in &fp.pads {
            let at = local(pad.position, placement);
            let angle = if placement.side == BoardSide::Bottom {
                180.0 - pad.rotation_mdeg as f64 / 1000.0
            } else {
                pad.rotation_mdeg as f64 / 1000.0
            } + f64::from(placement.rotation) * 90.0;
            let tech = match pad.technology {
                PadTechnology::Smd => "smd",
                PadTechnology::ThroughHole => "thru_hole",
                PadTechnology::NonPlated => "np_thru_hole",
            };
            let shape = match pad.shape {
                PadShape::Rect => "rect",
                PadShape::RoundRect => "roundrect",
                PadShape::Circle => "circle",
                PadShape::Oval => "oval",
                PadShape::Custom => "custom",
            };
            writeln!(
                out,
                "(pad {} {tech} {shape} (at {:.6} {:.6} {angle:.6}) (size {:.6} {:.6}) (layers {})",
                q(&pad.number),
                mm(at.x),
                mm(at.y),
                mm(pad.size.x),
                mm(pad.size.y),
                pad.layers
                    .iter()
                    .map(|l| q(&layer(l, placement.side)))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .map_err(|e| e.to_string())?;
            if let Some(drill) = pad.drill {
                if drill.x == drill.y {
                    writeln!(out, "(drill {:.6})", mm(drill.x))
                } else {
                    writeln!(out, "(drill oval {:.6} {:.6})", mm(drill.x), mm(drill.y))
                }
                .map_err(|e| e.to_string())?;
            }
            if pad.shape == PadShape::RoundRect {
                writeln!(
                    out,
                    "(roundrect_rratio {:.6})",
                    pad.roundrect_ratio_ppm as f64 / 1e6
                )
                .map_err(|e| e.to_string())?;
            }
            writeln!(out,"(solder_mask_margin {:.6}) (solder_paste_margin {:.6}) (solder_paste_margin_ratio {:.6})",mm(pad.mask_margin_nm),mm(pad.paste_margin_nm),pad.paste_ratio_ppm as f64/1e6).map_err(|e|e.to_string())?;
            if pad.shape == PadShape::Custom {
                out.push_str("(options (clearance outline) (anchor rect)) (primitives\n");
                for poly in &pad.polygons {
                    out.push_str("(gr_poly (pts ");
                    for p in poly {
                        write!(
                            out,
                            "(xy {:.6} {:.6})",
                            mm(p.x),
                            if placement.side == BoardSide::Bottom {
                                -mm(p.y)
                            } else {
                                mm(p.y)
                            }
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    out.push_str(") (width 0) (fill yes))\n");
                }
                out.push_str(")\n");
            }
            if pad.technology != PadTechnology::NonPlated
                && let Some(anchor) = mapping.get(&pad.number)
                && let Some(net) = netlist.net_for_pin(*anchor)
            {
                let name = netlist
                    .nets
                    .iter()
                    .find(|n| n.id == net)
                    .map_or("", |n| n.name.as_str());
                writeln!(out, "(net {} {})", net.0 + 1, q(name)).map_err(|e| e.to_string())?;
            }
            out.push_str(")\n");
        }
        for keepout in &fp.keepouts {
            out.push_str("(zone (net 0) (net_name \"\") (layers \"*.Cu\") (hatch edge 0.5) (connect_pads (clearance 0)) (min_thickness 0.25) ");
            let allowed = |v| if v { "not_allowed" } else { "allowed" };
            writeln!(out,"(keepout (tracks {}) (vias {}) (pads {}) (copperpour {}) (footprints not_allowed)) (fill (thermal_gap 0.5) (thermal_bridge_width 0.5)) (polygon (pts",allowed(keepout.tracks),allowed(keepout.vias),allowed(keepout.pads),allowed(keepout.copper)).map_err(|e|e.to_string())?;
            for p in &keepout.polygon {
                let p = local(*p, placement);
                writeln!(out, "(xy {:.6} {:.6})", mm(p.x), mm(p.y)).map_err(|e| e.to_string())?;
            }
            out.push_str(")))\n");
        }
        out.push_str(")\n");
    }
    for h in board.holes.values() {
        writeln!(out,
            "(footprint \"ProtoCraft:MountingHole\" (layer \"F.Cu\") (at {:.6} {:.6}) (attr exclude_from_pos_files exclude_from_bom) (fp_text reference \"H{}\" (at 0 {:.6}) (layer \"F.SilkS\") (effects (font (size 1 1) (thickness 0.15)))) (pad \"\" np_thru_hole circle (at 0 0) (size {:.6} {:.6}) (drill {:.6}) (layers \"*.Cu\" \"*.Mask\")))",
            mm(h.position.x), mm(h.position.y), h.id, -mm(h.drill_nm)/2.0-1.0,
            mm(h.drill_nm), mm(h.drill_nm), mm(h.drill_nm)
        ).map_err(|e|e.to_string())?;
    }
    for t in board.tracks.values() {
        writeln!(
            out,
            "(segment (start {:.6} {:.6}) (end {:.6} {:.6}) (width {:.6}) (layer {}) (net {}))",
            mm(t.start.x),
            mm(t.start.y),
            mm(t.end.x),
            mm(t.end.y),
            mm(t.width_nm),
            q(if t.layer == BoardSide::Top {
                "F.Cu"
            } else {
                "B.Cu"
            }),
            net_number(&netlist, t.net)?
        )
        .map_err(|e| e.to_string())?;
    }
    for v in board.vias.values() {
        writeln!(
            out,
            "(via (at {:.6} {:.6}) (size {:.6}) (drill {:.6}) (layers \"F.Cu\" \"B.Cu\") (net {}))",
            mm(v.position.x),
            mm(v.position.y),
            mm(v.diameter_nm),
            mm(v.drill_nm),
            net_number(&netlist, v.net)?
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(out,"(gr_rect (start 0 0) (end {:.6} {:.6}) (stroke (width 0.05) (type default)) (fill none) (layer \"Edge.Cuts\"))\n)",mm(board.width_nm),mm(board.height_nm)).map_err(|e|e.to_string())?;
    Ok(out)
}
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn export_svg(project: &Project) -> Result<String, String> {
    let pads = placed_pads(project)?;
    let b = project.board.as_ref().ok_or("PCB ausente")?;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#15372b\"/>",
        mm(b.width_nm),
        mm(b.height_nm),
        mm(b.width_nm),
        mm(b.height_nm)
    );
    for (id, placement) in &b.placements {
        let c = &project.components[id];
        let fp = c.footprint.as_ref().ok_or("Footprint ausente")?;
        for line in &fp.lines {
            if line.layer == "F.Fab" || line.layer == "Eco2.User" || line.layer == "F.SilkS" {
                let a = placement.transform(line.start);
                let z = placement.transform(line.end);
                write!(
                    out,
                    "<path d=\"M {} {} L {} {}\" stroke=\"#e7e9dd\" stroke-width=\"{}\"/>",
                    mm(a.x),
                    mm(a.y),
                    mm(z.x),
                    mm(z.y),
                    mm(line.width_nm)
                )
                .map_err(|e| e.to_string())?;
            }
        }
        for k in &fp.keepouts {
            write!(out,"<polygon points=\"{}\" fill=\"#ed6b5240\" stroke=\"#ed6b52\" stroke-width=\"0.1\"/>",k.polygon.iter().map(|p|{let p=placement.transform(*p);format!("{},{}",mm(p.x),mm(p.y))}).collect::<Vec<_>>().join(" ")).map_err(|e|e.to_string())?;
        }
        write!(
            out,
            "<text x=\"{}\" y=\"{}\" font-size=\"1\" fill=\"white\">{}</text>",
            mm(placement.position.x),
            mm(placement.position.y),
            xml(&c.reference)
        )
        .map_err(|e| e.to_string())?;
    }
    for pad in &pads {
        for polygon in &pad.polygons {
            let points = polygon
                .iter()
                .map(|p| format!("{},{}", mm(p.x), mm(p.y)))
                .collect::<Vec<_>>()
                .join(" ");
            write!(
                out,
                "<polygon points=\"{points}\" fill=\"{}\"><title>{}:{}</title></polygon>",
                if pad.layers & 1 != 0 {
                    "#f2c75c"
                } else {
                    "#74a8ed"
                },
                pad.component.0,
                xml(&pad.number)
            )
            .map_err(|e| e.to_string())?;
        }
        if let Some(drill) = pad.pad.drill {
            write!(
                out,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"#101718\"/>",
                mm(pad.position.x),
                mm(pad.position.y),
                mm(drill.x) / 2.0,
                mm(drill.y) / 2.0
            )
            .map_err(|e| e.to_string())?;
        }
    }
    for t in b.tracks.values() {
        write!(out,"<path d=\"M {} {} L {} {}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",mm(t.start.x),mm(t.start.y),mm(t.end.x),mm(t.end.y),if t.layer==BoardSide::Top {"#eb6568"}else{"#76a9f4"},mm(t.width_nm)).map_err(|e|e.to_string())?;
    }
    for v in b.vias.values() {
        write!(out,"<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#e5ce7b\"/><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#101718\"/>",mm(v.position.x),mm(v.position.y),mm(v.diameter_nm)/2.0,mm(v.position.x),mm(v.position.y),mm(v.drill_nm)/2.0).map_err(|e|e.to_string())?;
    }
    for h in b.holes.values() {
        write!(out,"<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#101718\" stroke=\"#d0d7da\" stroke-width=\"0.05\"/>",mm(h.position.x),mm(h.position.y),mm(h.drill_nm)/2.0).map_err(|e|e.to_string())?;
    }
    out.push_str("</svg>\n");
    Ok(out)
}
