//! Cost of a design in dollars (README section 9.2). Every figure comes from a catalog record.
//! A part without a price makes the total a lower bound and is named, never guessed.

use serde::Serialize;
use serde_json::Value;

use seatlock_core::{catalog, DesignFile};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostLine {
    pub item: String,
    pub quantity: f64,
    /// "each", "kg", or "piece".
    pub unit: String,
    pub unit_usd: Option<f64>,
    pub total_usd: Option<f64>,
    pub read_on: Option<String>,
    /// The figure came from somewhere other than the part's own page (README section 9.1).
    pub unconfirmed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostReport {
    /// 2 for a stereo pair, otherwise 1.
    pub speakers: u32,
    /// Quantities are for the whole system.
    pub lines: Vec<CostLine>,
    /// Sum of the lines that have a price.
    pub known_total_usd: f64,
    /// True when every line has a price and nothing is wrong.
    pub complete: bool,
    pub unconfirmed: bool,
    /// One plain-language sentence for the page.
    pub summary: String,
    pub problems: Vec<String>,
    /// One entry per stock piece bought: its cuts and its offcut.
    pub cut_list: Vec<String>,
    /// Printed plastic for the whole system.
    pub print_mass_kg: f64,
    /// Lower bound from the printer's maximum flow, for the whole system.
    pub print_time_h_at_least: Option<f64>,
}

/// What one speaker's cabinet is made of, from the geometry.
#[derive(Debug, Clone, Default)]
pub struct CabinetParts {
    /// Printed solid volume of one speaker, after cut-outs. `None` when the shape cannot say.
    pub print_solid_m3: Option<f64>,
    /// Stock cuts for one speaker: (stock id, cut length in m).
    pub stock_cuts: Vec<(String, f64)>,
    /// Flat panels for one speaker: (stock id, name, [a, b] in m).
    pub stock_panels: Vec<(String, String, [f64; 2])>,
    /// An assembly's parts with a material each (outer shape `mesh`): (part name, material,
    /// volume in m^3, true for damping). Empty when the whole print is the design's material.
    pub part_materials: Vec<(String, String, f64, bool)>,
    pub problems: Vec<String>,
}

fn price_of(record: &Value) -> (Option<f64>, Option<String>, bool) {
    let amount = record.pointer("/price/amount").and_then(Value::as_f64);
    let read_on = record
        .pointer("/price/read_on")
        .and_then(Value::as_str)
        .map(str::to_string);
    let via = record
        .pointer("/price/via")
        .and_then(Value::as_str)
        .is_some();
    (amount, read_on, via)
}

fn name_of(record: &Value, id: &str) -> String {
    if let Some(name) = record.get("name").and_then(Value::as_str) {
        if record.get("model").is_none() {
            return name.to_string();
        }
    }
    let maker = record.get("maker").and_then(Value::as_str).unwrap_or("");
    let model = record.get("model").and_then(Value::as_str).unwrap_or(id);
    format!("{maker} {model}").trim().to_string()
}

fn line(
    item: String,
    quantity: f64,
    unit: &str,
    unit_usd: Option<f64>,
    read_on: Option<String>,
    unconfirmed: bool,
) -> CostLine {
    CostLine {
        item,
        quantity,
        unit: unit.into(),
        unit_usd,
        total_usd: unit_usd.map(|price| price * quantity),
        read_on,
        unconfirmed,
    }
}

/// Lays cuts into whole pieces in the order given, first fit. Returns the cuts in each piece.
fn pack_cuts(cuts: &[f64], piece_m: f64) -> Vec<Vec<f64>> {
    let mut pieces: Vec<Vec<f64>> = Vec::new();
    for &cut in cuts {
        let slot = pieces
            .iter_mut()
            .find(|piece| piece.iter().sum::<f64>() + cut <= piece_m + 1e-9);
        match slot {
            Some(piece) => piece.push(cut),
            None => pieces.push(vec![cut]),
        }
    }
    pieces
}

fn amp_channels(id: &str, problems: &mut Vec<String>) -> Option<u32> {
    let record = catalog::amplifier(id);
    match record
        .as_ref()
        .and_then(|r| r.get("channels").and_then(Value::as_u64))
    {
        Some(count) if count > 0 => Some(count as u32),
        _ => {
            problems.push(format!(
                "Amplifier {id} has no channel count in the catalog."
            ));
            None
        }
    }
}

/// Lays panels `(label, long, short)` onto sheets `long_m` x `short_m` in rows across the long
/// side, tallest first, rotating a panel when that is the only way it fits. Saw kerf is ignored.
#[allow(clippy::type_complexity)]
fn pack_panels(
    panels: &[(String, f64, f64)],
    long_m: f64,
    short_m: f64,
) -> Result<Vec<Vec<(String, f64, f64)>>, String> {
    // Each sheet: rows of (height, used length), and the panels placed.
    struct Sheet {
        rows: Vec<(f64, f64)>,
        used_height: f64,
        placed: Vec<(String, f64, f64)>,
    }
    let mut order: Vec<&(String, f64, f64)> = panels.iter().collect();
    order.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let mut sheets: Vec<Sheet> = Vec::new();
    for panel in order {
        let (label, a, b) = panel;
        // Lay the panel with its long side along the sheet's long side if it fits; otherwise turned.
        let (along, across) = if *a <= long_m + 1e-9 && *b <= short_m + 1e-9 {
            (*a, *b)
        } else if *b <= long_m + 1e-9 && *a <= short_m + 1e-9 {
            (*b, *a)
        } else {
            return Err(format!(
                "A {:.0} x {:.0} mm panel is larger than the sheet sold",
                a * 1000.0,
                b * 1000.0
            ));
        };
        let mut done = false;
        for sheet in sheets.iter_mut() {
            if let Some(row) = sheet
                .rows
                .iter_mut()
                .find(|(height, used)| across <= *height + 1e-9 && used + along <= long_m + 1e-9)
            {
                row.1 += along;
                sheet.placed.push((label.clone(), *a, *b));
                done = true;
                break;
            }
            if sheet.used_height + across <= short_m + 1e-9 {
                sheet.rows.push((across, along));
                sheet.used_height += across;
                sheet.placed.push((label.clone(), *a, *b));
                done = true;
                break;
            }
        }
        if !done {
            sheets.push(Sheet {
                rows: vec![(across, along)],
                used_height: across,
                placed: vec![(label.clone(), *a, *b)],
            });
        }
    }
    Ok(sheets.into_iter().map(|sheet| sheet.placed).collect())
}

pub fn report(design: &DesignFile, parts: &CabinetParts) -> CostReport {
    let speakers: u32 = if design.build.stereo_pair { 2 } else { 1 };
    let n = speakers as f64;
    let mut lines = Vec::new();
    let mut problems = parts.problems.clone();
    let mut cut_list = Vec::new();

    // Drivers: one per radiator per speaker.
    let mut driver_ids: Vec<String> = Vec::new();
    for radiator in &design.speaker.radiators {
        if let Some(id) = radiator.fields.get("driver_id").and_then(Value::as_str) {
            driver_ids.push(id.to_string());
        }
    }
    let mut seen = Vec::new();
    for id in &driver_ids {
        if seen.contains(id) {
            continue;
        }
        seen.push(id.clone());
        let count = driver_ids.iter().filter(|other| *other == id).count() as f64 * n;
        match catalog::driver(id) {
            Some(record) => {
                let (amount, read_on, via) = price_of(&record);
                lines.push(line(
                    name_of(&record, id),
                    count,
                    "each",
                    amount,
                    read_on,
                    via,
                ));
            }
            None => problems.push(format!("Driver {id} is not in the catalog.")),
        }
    }

    // Passive radiators on ducts (README §5): one catalog part each per speaker.
    let mut pr_ids: Vec<String> = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        if duct.get("kind").and_then(Value::as_str) != Some("passive_radiator") {
            continue;
        }
        if let Some(id) = duct.get("passive_radiator_id").and_then(Value::as_str) {
            pr_ids.push(id.to_string());
        }
    }
    let mut pr_seen = Vec::new();
    for id in &pr_ids {
        if pr_seen.contains(id) {
            continue;
        }
        pr_seen.push(id.clone());
        let count = pr_ids.iter().filter(|other| *other == id).count() as f64 * n;
        match catalog::passive_radiator(id) {
            Some(record) => {
                let (amount, read_on, via) = price_of(&record);
                lines.push(line(
                    name_of(&record, id),
                    count,
                    "each",
                    amount,
                    read_on,
                    via,
                ));
            }
            None => problems.push(format!("Passive radiator {id} is not in the catalog.")),
        }
    }

    // Panel speakers: an exciter and a panel per radiator per speaker. A panel is priced per
    // kilogram of its material (README section 9.2); a record without a price says so.
    for radiator in &design.speaker.radiators {
        if let Some(id) = radiator.fields.get("exciter_id").and_then(Value::as_str) {
            match catalog::exciter(id) {
                Some(record) => {
                    let (amount, read_on, via) = price_of(&record);
                    lines.push(line(name_of(&record, id), n, "each", amount, read_on, via));
                }
                None => problems.push(format!("Exciter {id} is not in the catalog.")),
            }
        }
        let Some(panel) = radiator.fields.get("panel").and_then(Value::as_object) else {
            continue;
        };
        let size = |key: &str| panel.get(key).and_then(Value::as_f64).unwrap_or(0.0);
        // One material at the panel's thickness, or each layer of a stack by its own.
        let layers: Vec<(String, f64)> = match panel.get("layers").and_then(Value::as_array) {
            Some(list) => list
                .iter()
                .map(|layer| {
                    (
                        layer
                            .get("material")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        layer
                            .get("thickness_m")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                    )
                })
                .collect(),
            None => vec![(
                panel
                    .get("material")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                size("thickness_m"),
            )],
        };
        for (id, thickness) in layers {
            match catalog::panel_material(&id) {
                Some(record) => {
                    let density = record
                        .get("density_kg_m3")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let mass_kg = size("width_m") * size("height_m") * thickness * density * n;
                    let per_kg = record.pointer("/price/per_kg").and_then(Value::as_f64);
                    let (_, read_on, via) = price_of(&record);
                    let name = record
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    lines.push(line(
                        format!("Panel, {name}"),
                        mass_kg,
                        "kg",
                        per_kg,
                        read_on,
                        via,
                    ));
                }
                None => problems.push(format!("Panel material {id} is not in the catalog.")),
            }
        }
    }

    // Parts of an imported assembly with materials of their own: each printed part priced by
    // its material, its share of the print after cut-outs by its own volume; damping bought
    // by volume.
    let solid_parts: Vec<&(String, String, f64, bool)> =
        parts.part_materials.iter().filter(|p| !p.3).collect();
    let mut print_mass_kg = 0.0;
    let mut print_time_h_at_least = None;
    if !solid_parts.is_empty() {
        let raw: f64 = solid_parts.iter().map(|p| p.2).sum();
        let solid = parts.print_solid_m3.unwrap_or(raw).max(0.0);
        let scale = if raw > 0.0 { solid / raw } else { 0.0 };
        let mut printed_m3 = 0.0;
        for (name, id, volume, _) in solid_parts.iter().copied() {
            let Some(record) = catalog::material(id) else {
                problems.push(format!(
                    "Part {name}: material {id} is not a print material in the catalog."
                ));
                continue;
            };
            let Some(density) = record.get("density_kg_m3").and_then(Value::as_f64) else {
                problems.push(format!(
                    "Print material {id} has no density in the catalog."
                ));
                continue;
            };
            let mass = volume * scale * density * design.build.infill_fraction * n;
            print_mass_kg += mass;
            printed_m3 += volume * scale;
            lines.push(line(
                format!("Printed {}, {name}", id.to_uppercase()),
                mass,
                "kg",
                record.get("price_per_kg").and_then(Value::as_f64),
                record
                    .pointer("/price_source/read_on")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                false,
            ));
        }
        for (name, id, volume, _) in parts.part_materials.iter().filter(|p| p.3) {
            let record = catalog::all("catalog/damping/")
                .into_iter()
                .find(|r| r.get("id").and_then(Value::as_str) == Some(id.as_str()));
            let litres = volume * n * 1000.0;
            match record {
                Some(r) => lines.push(line(
                    format!("Damping, {name}: {}", name_of(&r, id)),
                    litres,
                    "L",
                    r.get("price_per_litre").and_then(Value::as_f64),
                    r.pointer("/price/read_on")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    r.pointer("/price/via").is_some(),
                )),
                None => lines.push(line(
                    format!(
                        "Damping, {name}: {}",
                        if id.is_empty() {
                            "material not chosen"
                        } else {
                            id
                        }
                    ),
                    litres,
                    "L",
                    None,
                    None,
                    false,
                )),
            }
        }
        let flow = catalog::printer(None)
            .and_then(|record| record.get("max_flow_m3_per_s").and_then(Value::as_f64));
        print_time_h_at_least =
            flow.map(|flow| printed_m3 * design.build.infill_fraction * n / flow / 3600.0);
    }

    // Chamber damping fill (`chambers[].fill`, catalog/damping/): bought by the chamber's
    // listed box volume in litres. Plastic placed in the chamber is not subtracted here; the
    // page can shrink the chamber if the fill only packs part of it.
    for chamber in &design.speaker.enclosure.chambers {
        let Some(id) = chamber.get("fill").and_then(Value::as_str) else {
            continue;
        };
        let name = chamber
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("chamber");
        let min = chamber.get("min_m").and_then(Value::as_array);
        let max = chamber.get("max_m").and_then(Value::as_array);
        let volume = match (min, max) {
            (Some(lo), Some(hi)) if lo.len() == 3 && hi.len() == 3 => {
                let mut v = 1.0;
                for i in 0..3 {
                    let a = lo[i].as_f64().unwrap_or(0.0);
                    let b = hi[i].as_f64().unwrap_or(0.0);
                    v *= (b - a).max(0.0);
                }
                v
            }
            _ => 0.0,
        };
        let litres = volume * n * 1000.0;
        match catalog::damping(id) {
            Some(r) => lines.push(line(
                format!("Damping fill, {name}: {}", name_of(&r, id)),
                litres,
                "L",
                r.get("price_per_litre").and_then(Value::as_f64),
                r.pointer("/price/read_on")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                r.pointer("/price/via").is_some(),
            )),
            None => problems.push(format!(
                "Chamber {name}: fill {id} is not a damping material in the catalog."
            )),
        }
    }

    // Printed parts.
    let material = catalog::material(&design.build.material);
    let density = material
        .as_ref()
        .and_then(|record| record.get("density_kg_m3").and_then(Value::as_f64));
    if !solid_parts.is_empty() {
        // Priced part by part above.
    } else if let Some(whole) = parts.print_solid_m3.filter(|solid| *solid > 0.0) {
        // A wall build (README section 6.2): two thin printed skins with a core between them,
        // poured (sand, gypsum cement) or printed in a second filament. The core's share of
        // the print is estimated from the wall thickness.
        let wall = wall_build(design, whole, &mut problems);
        let solid = whole - wall.as_ref().map_or(0.0, |w| w.core_m3);
        if let Some(wall) = &wall {
            lines.push(line(
                wall.name.clone(),
                wall.core_m3 * wall.kg_per_m3 * n,
                "kg",
                wall.per_kg,
                wall.read_on.clone(),
                wall.via,
            ));
            if wall.printed {
                print_mass_kg += wall.core_m3 * wall.kg_per_m3 * n;
            }
        }
        match density {
            Some(density) => {
                print_mass_kg += solid * density * design.build.infill_fraction * n;
                let per_kg = material
                    .as_ref()
                    .and_then(|record| record.get("price_per_kg").and_then(Value::as_f64));
                let read_on = material
                    .as_ref()
                    .and_then(|record| record.pointer("/price_source/read_on"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                lines.push(line(
                    format!("Printed {}", design.build.material.to_uppercase()),
                    solid * density * design.build.infill_fraction * n,
                    "kg",
                    per_kg,
                    read_on,
                    false,
                ));
                let flow = catalog::printer(None)
                    .and_then(|record| record.get("max_flow_m3_per_s").and_then(Value::as_f64));
                let printed_m3 = solid
                    + wall
                        .as_ref()
                        .filter(|w| w.printed)
                        .map_or(0.0, |w| w.core_m3);
                print_time_h_at_least =
                    flow.map(|flow| printed_m3 * design.build.infill_fraction * n / flow / 3600.0);
            }
            None => problems.push(format!(
                "Print material {} has no density in the catalog.",
                design.build.material
            )),
        }
    } else if parts.print_solid_m3.is_none() {
        problems.push("The cabinet's printed volume is not known for this shape.".into());
    }

    // Stock parts: whole pieces, cuts laid in order.
    let mut stock_ids: Vec<String> = Vec::new();
    for (id, _) in &parts.stock_cuts {
        if !stock_ids.contains(id) {
            stock_ids.push(id.clone());
        }
    }
    for id in &stock_ids {
        let one: Vec<f64> = parts
            .stock_cuts
            .iter()
            .filter(|(other, _)| other == id)
            .map(|(_, length)| *length)
            .collect();
        let cuts: Vec<f64> = (0..speakers).flat_map(|_| one.clone()).collect();
        let Some(record) = catalog::stock(id) else {
            problems.push(format!("Stock part {id} is not in the catalog."));
            continue;
        };
        let name = name_of(&record, id);
        let Some(piece_m) = record.get("length_m").and_then(Value::as_f64) else {
            problems.push(format!("{name} has no sold length in the catalog."));
            continue;
        };
        if let Some(cut) = cuts.iter().find(|cut| **cut > piece_m + 1e-9) {
            problems.push(format!(
                "A {cut:.3} m cut is longer than the {piece_m:.3} m piece of {name} that is sold."
            ));
            let (amount, read_on, via) = price_of(&record);
            let mut bad = line(name, 0.0, "piece", amount, read_on, via);
            bad.total_usd = None;
            lines.push(bad);
            continue;
        }
        let pieces = pack_cuts(&cuts, piece_m);
        for (index, piece) in pieces.iter().enumerate() {
            let used: f64 = piece.iter().sum();
            let listed: Vec<String> = piece.iter().map(|cut| format!("{cut:.3} m")).collect();
            cut_list.push(format!(
                "{name}, piece {}: cuts {}; offcut {:.3} m",
                index + 1,
                listed.join(", "),
                piece_m - used
            ));
        }
        let (amount, read_on, via) = price_of(&record);
        lines.push(line(
            name,
            pieces.len() as f64,
            "piece",
            amount,
            read_on,
            via,
        ));
    }

    // Sheet stock: panels laid onto whole sheets in rows, largest first.
    let mut sheet_ids: Vec<String> = Vec::new();
    for (id, _, _) in &parts.stock_panels {
        if !sheet_ids.contains(id) {
            sheet_ids.push(id.clone());
        }
    }
    for id in &sheet_ids {
        let Some(record) = catalog::stock(id) else {
            problems.push(format!("Stock part {id} is not in the catalog."));
            continue;
        };
        let name = name_of(&record, id);
        let sheet = (
            record.get("width_m").and_then(Value::as_f64),
            record.get("length_m").and_then(Value::as_f64),
        );
        let (Some(sheet_w), Some(sheet_l)) = sheet else {
            problems.push(format!("{name} has no sheet size in the catalog."));
            continue;
        };
        let mut panels: Vec<(String, f64, f64)> = Vec::new();
        for speaker in 0..speakers {
            for (other, panel, [a, b]) in &parts.stock_panels {
                if other == id {
                    let label = if speakers > 1 {
                        format!("{panel} {}", speaker + 1)
                    } else {
                        panel.clone()
                    };
                    panels.push((label, a.max(*b), a.min(*b)));
                }
            }
        }
        match pack_panels(&panels, sheet_w.max(sheet_l), sheet_w.min(sheet_l)) {
            Ok(sheets) => {
                for (index, sheet) in sheets.iter().enumerate() {
                    let listed: Vec<String> = sheet
                        .iter()
                        .map(|(label, a, b)| {
                            format!("{label} {:.0} x {:.0} mm", a * 1000.0, b * 1000.0)
                        })
                        .collect();
                    cut_list.push(format!(
                        "{name}, sheet {}: {}",
                        index + 1,
                        listed.join("; ")
                    ));
                }
                let (amount, read_on, via) = price_of(&record);
                lines.push(line(
                    name,
                    sheets.len() as f64,
                    "sheet",
                    amount,
                    read_on,
                    via,
                ));
            }
            Err(err) => problems.push(format!("{err} ({name})")),
        }
    }

    // Extras, per speaker.
    if design.build.extras_cost > 0.0 {
        lines.push(line(
            "Extras".into(),
            n,
            "each",
            Some(design.build.extras_cost),
            None,
            false,
        ));
    }

    // Electronics: every radiator in the system gets its own amplifier channel.
    let needed = design.speaker.radiators.len() as u32 * speakers;
    let mut bought: Vec<(String, u32)> = Vec::new();
    let mut channels = 0u32;
    for id in &design.electronics.amplifiers {
        if let Some(count) = amp_channels(id, &mut problems) {
            channels += count;
            match bought.iter_mut().find(|(other, _)| other == id) {
                Some((_, units)) => *units += 1,
                None => bought.push((id.clone(), 1)),
            }
        }
    }
    if design.electronics.amplifiers.is_empty() {
        problems.push("No amplifier is chosen.".into());
    } else if channels < needed {
        let first = design.electronics.amplifiers[0].clone();
        if let Some(count) = amp_channels(&first, &mut Vec::new()) {
            let more = (needed - channels).div_ceil(count);
            if let Some((_, units)) = bought.iter_mut().find(|(other, _)| *other == first) {
                *units += more;
            }
        }
    }
    for (id, units) in &bought {
        let record = catalog::amplifier(id).unwrap_or(Value::Null);
        let (amount, read_on, via) = price_of(&record);
        lines.push(line(
            name_of(&record, id),
            *units as f64,
            "each",
            amount,
            read_on,
            via,
        ));
    }
    if let Some(id) = &design.electronics.dsp_device {
        match catalog::amplifier(id) {
            Some(record) => {
                let built_into = record.get("built_into").and_then(Value::as_str);
                if built_into.is_some_and(|host| bought.iter().any(|(other, _)| other == host)) {
                    // The processor is on an amplifier board already bought.
                } else {
                    let per = record
                        .get("channels")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        .max(1) as u32;
                    let (amount, read_on, via) = price_of(&record);
                    lines.push(line(
                        name_of(&record, id),
                        needed.div_ceil(per) as f64,
                        "each",
                        amount,
                        read_on,
                        via,
                    ));
                }
            }
            None => problems.push(format!("Processor {id} is not in the catalog.")),
        }
    }

    let known_total_usd: f64 = lines.iter().filter_map(|line| line.total_usd).sum();
    let unpriced: Vec<String> = lines
        .iter()
        .filter(|line| line.total_usd.is_none())
        .map(|line| line.item.clone())
        .collect();
    let unconfirmed = lines.iter().any(|line| line.unconfirmed);
    let complete = unpriced.is_empty() && problems.is_empty();
    let pair = if speakers == 2 {
        "a stereo pair"
    } else {
        "one speaker"
    };
    let mut summary = if complete {
        format!("${known_total_usd:.2} for {pair}.")
    } else if unpriced.is_empty() {
        format!("At least ${known_total_usd:.2} for {pair}; see the problems listed.")
    } else {
        format!(
            "At least ${known_total_usd:.2} for {pair}. No price yet for: {}.",
            unpriced.join(", ")
        )
    };
    if unconfirmed {
        summary.push_str(" Includes an unconfirmed price.");
    }
    CostReport {
        speakers,
        lines,
        known_total_usd,
        complete,
        unconfirmed,
        summary,
        problems,
        cut_list,
        print_mass_kg,
        print_time_h_at_least,
    }
}

/// The core of a wall build, for one speaker.
struct WallCore {
    name: String,
    core_m3: f64,
    /// Mass bought or printed per cubic meter of core.
    kg_per_m3: f64,
    per_kg: Option<f64>,
    read_on: Option<String>,
    via: bool,
    /// Printed in a second filament (counts as printed plastic and print time).
    printed: bool,
}

/// `build.wall`: `{ "kind": "filled", "skin_m": 0.0016, "fill": "<catalog/fills id>" }` or
/// `{ "kind": "two_material", "skin_m": 0.0016, "core": "<print material id>" }`.
fn wall_build(design: &DesignFile, solid_m3: f64, problems: &mut Vec<String>) -> Option<WallCore> {
    let wall = design.build.extra.get("wall")?.as_object()?;
    let kind = wall.get("kind").and_then(Value::as_str).unwrap_or("solid");
    if kind == "solid" {
        return None;
    }
    let skin = wall.get("skin_m").and_then(Value::as_f64).unwrap_or(0.0016);
    let Some(wall_m) = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("wall_m")
        .and_then(Value::as_f64)
    else {
        problems
            .push("A wall build needs a printed cabinet with a wall thickness (wall_m).".into());
        return None;
    };
    if !(skin > 0.0 && 2.0 * skin < wall_m) {
        problems.push(format!(
            "The wall build's two {:.1} mm skins leave no room inside a {:.1} mm wall.",
            skin * 1000.0,
            wall_m * 1000.0
        ));
        return None;
    }
    let core_m3 = solid_m3 * (1.0 - 2.0 * skin / wall_m);
    let text = |record: &Value, key: &str| {
        record
            .pointer(key)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    match kind {
        "filled" => {
            let id = wall.get("fill").and_then(Value::as_str).unwrap_or("");
            let Some(record) = catalog::fill(id) else {
                problems.push(format!("Wall fill {id} is not in the catalog."));
                return None;
            };
            let kg_per_m3 = record
                .get("buy_kg_per_m3")
                .or_else(|| record.get("density_kg_m3"))
                .and_then(Value::as_f64)?;
            Some(WallCore {
                name: format!(
                    "Wall fill, {} (between {:.1} mm skins; volume estimated from the wall)",
                    name_of(&record, id),
                    skin * 1000.0
                ),
                core_m3,
                kg_per_m3,
                per_kg: record.pointer("/price/per_kg").and_then(Value::as_f64),
                read_on: text(&record, "/price/read_on"),
                via: record.pointer("/price/via").is_some(),
                printed: false,
            })
        }
        "two_material" => {
            let id = wall.get("core").and_then(Value::as_str).unwrap_or("");
            let Some(record) = catalog::material(id) else {
                problems.push(format!(
                    "Wall core {id} is not a print material in the catalog."
                ));
                return None;
            };
            let kg_per_m3 = record.get("density_kg_m3").and_then(Value::as_f64)?;
            let name = record
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id)
                .to_string();
            Some(WallCore {
                name: format!(
                    "Printed wall core, {name} (between {:.1} mm skins; volume estimated from the wall)",
                    skin * 1000.0
                ),
                core_m3,
                kg_per_m3: kg_per_m3 * design.build.infill_fraction,
                per_kg: record.get("price_per_kg").and_then(Value::as_f64),
                read_on: text(&record, "/price_source/read_on"),
                via: record.pointer("/price_source/via").is_some(),
                printed: true,
            })
        }
        other => {
            problems.push(format!(
                "A wall build is solid, filled, or two_material, not {other}."
            ));
            None
        }
    }
}
