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

    // Printed parts.
    let material = catalog::material(&design.build.material);
    let density = material
        .as_ref()
        .and_then(|record| record.get("density_kg_m3").and_then(Value::as_f64));
    let mut print_mass_kg = 0.0;
    let mut print_time_h_at_least = None;
    if let Some(solid) = parts.print_solid_m3.filter(|solid| *solid > 0.0) {
        match density {
            Some(density) => {
                print_mass_kg = solid * density * design.build.infill_fraction * n;
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
                    print_mass_kg,
                    "kg",
                    per_kg,
                    read_on,
                    false,
                ));
                let flow = catalog::printer(None)
                    .and_then(|record| record.get("max_flow_m3_per_s").and_then(Value::as_f64));
                print_time_h_at_least =
                    flow.map(|flow| solid * design.build.infill_fraction * n / flow / 3600.0);
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
