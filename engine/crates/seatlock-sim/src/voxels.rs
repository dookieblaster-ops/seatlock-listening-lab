//! The 3D solver's grid for a builder cabinet (README section 5.5): the cabinet's air from the
//! outer shape and its chambers, printed ducts with their bores and tubes, the driver's body,
//! the source patch behind the cone, and an exterior air block around every opening.

use seatlock_core::registry::{MountPose, OuterShape};
use seatlock_fdtd::{Grid, EXTERIOR_WIDTHS};

use crate::chambers::Chamber;
use crate::ports::Bore;

const VOID: u8 = 0;
const SOLID: u8 = 1;
const INSIDE: u8 = 2;
const OUTSIDE: u8 = 3;
const NO_DUCT: u16 = u16::MAX;

/// Grids larger than this are refused: about 1 GB of working memory in the browser.
pub const MAX_CELLS: usize = 12_000_000;

/// Where the driver sits, for the source patch and its body.
pub struct DriverPlacement {
    pub pose: MountPose,
    pub wall_m: f64,
    pub piston_radius_m: f64,
    /// The driver's body is a solid cylinder of this radius and length behind the wall, so the
    /// grid's air matches the net volume the quick model uses.
    pub body_radius_m: f64,
    pub body_length_m: f64,
}

pub struct CabinetGrid {
    pub grid: Grid,
    /// The duct ids of the openings to outside, in the grid's opening order.
    pub port_ids: Vec<String>,
    pub notes: Vec<String>,
    /// Air inside the cabinet on the grid.
    pub inside_volume_m3: f64,
    /// Longest side of the cabinet's air, for the self-check's frequency limit.
    pub longest_m: f64,
    /// True for the cells of chamber air: inside the cabinet and not in a duct's bore.
    pub chamber_air: Vec<bool>,
    /// Faces in damping material: `(axis, face slot, flow resistivity in Pa*s/m^2)`, the
    /// mean of the two cells' figures (README 5.5).
    pub damping: Vec<(u8, usize, f32)>,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The grid axis a direction runs along, when it runs along one.
fn grid_axis(dir: [f64; 3]) -> Option<usize> {
    (0..3).find(|axis| dir[*axis].abs() > 0.9999)
}

struct Lattice {
    n: [usize; 3],
    dx: f64,
    origin: [f64; 3],
}

impl Lattice {
    fn index(&self, c: [usize; 3]) -> usize {
        (c[0] * self.n[1] + c[1]) * self.n[2] + c[2]
    }
    fn center(&self, c: [usize; 3]) -> [f64; 3] {
        [
            self.origin[0] + (c[0] as f64 + 0.5) * self.dx,
            self.origin[1] + (c[1] as f64 + 0.5) * self.dx,
            self.origin[2] + (c[2] as f64 + 0.5) * self.dx,
        ]
    }
    fn coords(&self, index: usize) -> [usize; 3] {
        let k = index % self.n[2];
        let j = (index / self.n[2]) % self.n[1];
        let i = index / (self.n[1] * self.n[2]);
        [i, j, k]
    }
    fn cell_of(&self, p: [f64; 3]) -> Option<[usize; 3]> {
        let mut c = [0usize; 3];
        for axis in 0..3 {
            let f = ((p[axis] - self.origin[axis]) / self.dx).floor();
            if f < 0.0 || f as usize >= self.n[axis] {
                return None;
            }
            c[axis] = f as usize;
        }
        Some(c)
    }
}

/// Marks the cells of a straight shape along a grid axis, slice by slice, keeping each slice's
/// cell count equal to the true cross-section (README 5.5: preserve cross-section areas).
/// `measure(p)` gives `(t, normalized distance)` and `area(t)` the true area at `t`.
#[allow(clippy::too_many_arguments)]
fn mark_preserving_area(
    lattice: &Lattice,
    axis: usize,
    start: [f64; 3],
    dir: [f64; 3],
    length: f64,
    reach_m: f64,
    measure: &dyn Fn([f64; 3]) -> (f64, f64),
    area: &dyn Fn(f64) -> f64,
    mut mark: impl FnMut(usize),
) {
    let dx = lattice.dx;
    let (lo, hi) = {
        let a = start[axis];
        let b = start[axis] + dir[axis] * length;
        (a.min(b), a.max(b))
    };
    let first = ((lo - lattice.origin[axis]) / dx).floor().max(0.0) as usize;
    let last = (((hi - lattice.origin[axis]) / dx).ceil() as usize).min(lattice.n[axis]);
    for slice in first..last {
        let center_a = lattice.origin[axis] + (slice as f64 + 0.5) * dx;
        let t = (center_a - start[axis]) * dir[axis].signum();
        if t <= 0.0 || t >= length {
            continue;
        }
        let on_axis = [
            start[0] + dir[0] * t,
            start[1] + dir[1] * t,
            start[2] + dir[2] * t,
        ];
        let others: Vec<usize> = (0..3).filter(|other| *other != axis).collect();
        let mut range = [(0usize, 0usize); 2];
        for (slot, other) in others.iter().enumerate() {
            let a = ((on_axis[*other] - reach_m - lattice.origin[*other]) / dx)
                .floor()
                .max(0.0) as usize;
            let b = (((on_axis[*other] + reach_m - lattice.origin[*other]) / dx).ceil() as usize)
                .min(lattice.n[*other]);
            range[slot] = (a, b);
        }
        let mut ranked: Vec<(f64, usize)> = Vec::new();
        for u in range[0].0..range[0].1 {
            for v in range[1].0..range[1].1 {
                let mut c = [0usize; 3];
                c[axis] = slice;
                c[others[0]] = u;
                c[others[1]] = v;
                let (_, distance) = measure(lattice.center(c));
                ranked.push((distance, lattice.index(c)));
            }
        }
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let wanted = (area(t) / (dx * dx)).round().max(1.0) as usize;
        for (_, index) in ranked.into_iter().take(wanted) {
            mark(index);
        }
    }
}

pub fn cabinet_grid(
    shape: &dyn OuterShape,
    chambers: &[Chamber],
    bores: &[Bore],
    placed: &crate::solids::Placed,
    driver: &DriverPlacement,
    dx: f64,
) -> Result<CabinetGrid, String> {
    let size = shape.outer_size_m()?;
    shape.contains_cabinet([size[0] * 0.5, size[1] * 0.5, size[2] * 0.5])?;
    let mut notes = Vec::new();

    // Exterior blocks: EXTERIOR_WIDTHS opening widths past each mouth in every direction.
    let mut blocks: Vec<([f64; 3], [f64; 3])> = Vec::new();
    let mut mouths: Vec<[f64; 3]> = Vec::new();
    for bore in bores.iter().filter(|bore| bore.to_outside) {
        let width = bore.width_m();
        let reach = EXTERIOR_WIDTHS * width + 0.5 * width;
        let mouth = bore.mouth_m();
        mouths.push(mouth);
        blocks.push((
            [mouth[0] - reach, mouth[1] - reach, mouth[2] - reach],
            [mouth[0] + reach, mouth[1] + reach, mouth[2] + reach],
        ));
        if width < 2.0 * dx {
            notes.push(format!(
                "Port {} is narrower than two cells at {:.0} mm, so the 3D grid does not resolve it.",
                bore.id,
                dx * 1000.0
            ));
        }
    }
    let mut lo = [0.0_f64; 3];
    let mut hi = size;
    for (min, max) in &blocks {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(min[axis]);
            hi[axis] = hi[axis].max(max[axis]);
        }
    }
    // Shift the grid along each axis so that the cells across the cabinet's air, on a line
    // through its middle, add up to the air's true length (README 5.5: preserve sizes).
    let in_air = |p: [f64; 3]| -> Result<bool, String> {
        Ok(shape.contains_air(p)?
            && (chambers.is_empty() || chambers.iter().any(|chamber| chamber.contains(p))))
    };
    let middle = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let mut shift = [0.0_f64; 3];
    for axis in 0..3 {
        let fine = dx / 50.0;
        let samples = (size[axis] / fine).ceil() as usize;
        let mut inside_at = Vec::with_capacity(samples);
        for index in 0..samples {
            let mut p = middle;
            p[axis] = (index as f64 + 0.5) * fine;
            inside_at.push(in_air(p)?);
        }
        let true_length = inside_at.iter().filter(|x| **x).count() as f64 * fine;
        let mut best = (f64::MAX, 0.0);
        // Half-step offsets keep cell centers off round-number walls, where ties would make
        // the count depend on rounding.
        for step in 0..20 {
            let s = (step as f64 + 0.5) * dx / 20.0;
            let mut count = 0usize;
            let mut center = s + 0.5 * dx;
            while center < size[axis] {
                let mut p = middle;
                p[axis] = center;
                if in_air(p)? {
                    count += 1;
                }
                center += dx;
            }
            let error = (count as f64 * dx - true_length).abs();
            if error < best.0 - 1e-12 {
                best = (error, s);
            }
        }
        shift[axis] = best.1;
    }
    let origin = [
        ((lo[0] - shift[0]) / dx).floor() * dx + shift[0],
        ((lo[1] - shift[1]) / dx).floor() * dx + shift[1],
        ((lo[2] - shift[2]) / dx).floor() * dx + shift[2],
    ];
    let n = [
        ((hi[0] - origin[0]) / dx - 1e-9).ceil().max(1.0) as usize,
        ((hi[1] - origin[1]) / dx - 1e-9).ceil().max(1.0) as usize,
        ((hi[2] - origin[2]) / dx - 1e-9).ceil().max(1.0) as usize,
    ];
    let cells = n[0] * n[1] * n[2];
    if cells > MAX_CELLS {
        return Err(format!(
            "the grid would have {:.1} million cells at {:.0} mm, more than the {:.0} million this \
             browser solver allows.",
            cells as f64 / 1e6,
            dx * 1000.0,
            MAX_CELLS as f64 / 1e6
        ));
    }
    let lattice = Lattice { n, dx, origin };

    // Bores, slice by slice with their true areas, and their printed tubes.
    let mut duct_of = vec![NO_DUCT; cells];
    let mut solid_mark = vec![false; cells];
    for (index, bore) in bores.iter().enumerate() {
        let reach = bore.width_m() + 2.0 * bore.tube_m + 2.0 * dx;
        match grid_axis(bore.dir) {
            Some(axis) => mark_preserving_area(
                &lattice,
                axis,
                bore.inner,
                bore.dir,
                bore.length,
                reach,
                &|p| bore.locate(p, 0.0),
                &|t| bore.area_at(t),
                |cell| duct_of[cell] = index as u16,
            ),
            None => {
                notes.push(format!(
                    "Port {} does not run along a grid axis, so its cells are not sized to its \
                     true area.",
                    bore.id
                ));
                for (cell, duct) in duct_of.iter_mut().enumerate() {
                    let (t, distance) = bore.locate(lattice.center(lattice.coords(cell)), 0.0);
                    if t > 0.0 && t < bore.length && distance <= 1.0 {
                        *duct = index as u16;
                    }
                }
            }
        }
        if bore.moulded {
            continue;
        }
        for cell in 0..cells {
            if duct_of[cell] != NO_DUCT {
                continue;
            }
            let p = lattice.center(lattice.coords(cell));
            let (t, distance) = bore.locate(p, bore.tube_m);
            if t > 0.0 && t < bore.length && distance <= 1.0 && shape.contains_cabinet(p)? {
                solid_mark[cell] = true;
            }
        }
    }

    // The driver's body: a solid cylinder behind the wall, sized to its displacement.
    let inward = [
        -driver.pose.outward[0],
        -driver.pose.outward[1],
        -driver.pose.outward[2],
    ];
    let behind_wall = [
        driver.pose.position_m[0] + inward[0] * driver.wall_m,
        driver.pose.position_m[1] + inward[1] * driver.wall_m,
        driver.pose.position_m[2] + inward[2] * driver.wall_m,
    ];
    let body_measure = |p: [f64; 3]| -> (f64, f64) {
        let rel = [
            p[0] - behind_wall[0],
            p[1] - behind_wall[1],
            p[2] - behind_wall[2],
        ];
        let t = dot(rel, inward);
        let across = [
            rel[0] - inward[0] * t,
            rel[1] - inward[1] * t,
            rel[2] - inward[2] * t,
        ];
        (
            t,
            dot(across, across).sqrt() / driver.body_radius_m.max(1e-9),
        )
    };
    if driver.body_length_m > 0.0 && driver.body_radius_m > 0.0 {
        let area = std::f64::consts::PI * driver.body_radius_m.powi(2);
        match grid_axis(inward) {
            Some(axis) => mark_preserving_area(
                &lattice,
                axis,
                behind_wall,
                inward,
                driver.body_length_m,
                driver.body_radius_m + 2.0 * dx,
                &body_measure,
                &|_| area,
                |cell| solid_mark[cell] = true,
            ),
            None => {
                for (cell, solid) in solid_mark.iter_mut().enumerate() {
                    let (t, distance) = body_measure(lattice.center(lattice.coords(cell)));
                    if t > 0.0 && t < driver.body_length_m && distance <= 1.0 {
                        *solid = true;
                    }
                }
            }
        }
    }

    // Classify every cell.
    let mut kind = vec![VOID; cells];
    for (cell, slot) in kind.iter_mut().enumerate() {
        if duct_of[cell] != NO_DUCT {
            *slot = INSIDE;
            continue;
        }
        let p = lattice.center(lattice.coords(cell));
        if shape.contains_cabinet(p)? {
            let air = !solid_mark[cell]
                && !placed.contains(p)
                && shape.contains_air(p)?
                && (chambers.is_empty() || chambers.iter().any(|chamber| chamber.contains(p)));
            *slot = if air { INSIDE } else { SOLID };
        } else if blocks
            .iter()
            .any(|(min, max)| (0..3).all(|axis| p[axis] > min[axis] && p[axis] < max[axis]))
        {
            *slot = OUTSIDE;
        }
    }

    let neighbors = |cell: usize| -> [Option<(usize, usize, bool)>; 6] {
        // (neighbor, axis, neighbor is on the high side)
        let c = lattice.coords(cell);
        let mut out = [None; 6];
        for axis in 0..3 {
            if c[axis] > 0 {
                let mut d = c;
                d[axis] -= 1;
                out[2 * axis] = Some((lattice.index(d), axis, false));
            }
            if c[axis] + 1 < n[axis] {
                let mut d = c;
                d[axis] += 1;
                out[2 * axis + 1] = Some((lattice.index(d), axis, true));
            }
        }
        out
    };

    // A printed tube's wall is usually thinner than a cell. Keep the duct closed along its
    // length: every inside cell beside a bore cell, and within the duct's length, is solid.
    let mut tube_cells = 0usize;
    for cell in 0..cells {
        let duct = duct_of[cell];
        if duct == NO_DUCT {
            continue;
        }
        let bore = &bores[duct as usize];
        if bore.moulded {
            continue;
        }
        for (other, _, _) in neighbors(cell).iter().flatten() {
            if kind[*other] != INSIDE || duct_of[*other] != NO_DUCT {
                continue;
            }
            let (t, _) = bore.locate(lattice.center(lattice.coords(*other)), 0.0);
            if t > 0.0 && t < bore.length {
                kind[*other] = SOLID;
                tube_cells += 1;
            }
        }
    }
    if tube_cells > 0 && bores.iter().any(|bore| bore.tube_m < dx) {
        notes.push(format!(
            "Duct walls are one {:.0} mm cell thick on the grid, thicker than printed.",
            dx * 1000.0
        ));
    }

    // A wall thinner than a cell can leave inside air touching outside air away from any
    // duct. Close such gaps by making the outside cell solid.
    let mut thickened = 0usize;
    for cell in 0..cells {
        if kind[cell] != OUTSIDE {
            continue;
        }
        let leaks = neighbors(cell).iter().flatten().any(|(other, _, _)| {
            kind[*other] == INSIDE
                && (duct_of[*other] == NO_DUCT || !bores[duct_of[*other] as usize].to_outside)
        });
        if leaks {
            kind[cell] = SOLID;
            thickened += 1;
        }
    }
    if thickened > 0 {
        notes.push(format!(
            "Walls thinner than a {:.0} mm cell were thickened by one cell in {thickened} places.",
            dx * 1000.0
        ));
    }

    // Openings: faces between a port's bore and outside air.
    let port_indices: Vec<usize> = (0..bores.len())
        .filter(|index| bores[*index].to_outside)
        .collect();
    let mut openings: Vec<Vec<(u8, usize, f64)>> = vec![Vec::new(); port_indices.len()];
    let mut absorbing = Vec::new();
    // Face slots use the index of the cell on the face's high side (seatlock_fdtd::Grid).
    let grid_face = |_axis: usize, c: [usize; 3]| -> usize { lattice.index(c) };
    for cell in 0..cells {
        if kind[cell] != OUTSIDE {
            continue;
        }
        let c = lattice.coords(cell);
        let mut open = 0u8;
        for (side, neighbor) in neighbors(cell).iter().enumerate() {
            match neighbor {
                None => open += 1,
                Some((other, axis, high)) => {
                    if kind[*other] == VOID {
                        open += 1;
                    } else if kind[*other] == INSIDE {
                        let duct = duct_of[*other];
                        if let Some(slot) = port_indices.iter().position(|p| *p as u16 == duct) {
                            // Positive flow leaves the cabinet: toward this outside cell.
                            let (face, sign) = if *high {
                                // The bore is above this cell: flow toward -axis leaves.
                                (grid_face(*axis, lattice.coords(*other)), -1.0)
                            } else {
                                (grid_face(*axis, c), 1.0)
                            };
                            let _ = side;
                            openings[slot].push((*axis as u8, face, sign));
                        }
                    }
                }
            }
        }
        if open > 0 {
            let p = lattice.center(c);
            let r = mouths
                .iter()
                .map(|m| {
                    ((p[0] - m[0]).powi(2) + (p[1] - m[1]).powi(2) + (p[2] - m[2]).powi(2)).sqrt()
                })
                .fold(f64::MAX, f64::min);
            absorbing.push((cell, open, r));
        }
    }
    for (slot, faces) in openings.iter().enumerate() {
        if faces.is_empty() {
            return Err(format!(
                "port {} does not reach the outside air on the grid.",
                bores[port_indices[slot]].id
            ));
        }
    }

    // The source: inside air against the wall or the driver's body, within the cone's radius.
    let axis_out = (0..3)
        .max_by(|a, b| {
            driver.pose.outward[*a]
                .abs()
                .total_cmp(&driver.pose.outward[*b].abs())
        })
        .unwrap_or(1);
    let step_up = driver.pose.outward[axis_out] > 0.0;
    let reach = driver.wall_m + driver.body_length_m + 2.0 * dx;
    let mut source = Vec::new();
    for cell in 0..cells {
        if kind[cell] != INSIDE || duct_of[cell] != NO_DUCT {
            continue;
        }
        let p = lattice.center(lattice.coords(cell));
        let rel = [
            p[0] - driver.pose.position_m[0],
            p[1] - driver.pose.position_m[1],
            p[2] - driver.pose.position_m[2],
        ];
        let depth = -dot(rel, driver.pose.outward);
        if depth <= 0.0 || depth > reach {
            continue;
        }
        let across = [
            rel[0] + driver.pose.outward[0] * depth,
            rel[1] + driver.pose.outward[1] * depth,
            rel[2] + driver.pose.outward[2] * depth,
        ];
        if dot(across, across).sqrt() > driver.piston_radius_m {
            continue;
        }
        let mut c = lattice.coords(cell);
        let outer = if step_up {
            c[axis_out] += 1;
            (c[axis_out] < n[axis_out]).then(|| lattice.index(c))
        } else if c[axis_out] > 0 {
            c[axis_out] -= 1;
            Some(lattice.index(c))
        } else {
            None
        };
        if outer.is_none_or(|other| kind[other] == SOLID) {
            source.push(cell);
        }
    }
    if source.is_empty() {
        // A cone smaller than a cell: take the inside cell nearest the mount.
        let near = lattice
            .cell_of(behind_wall)
            .map(|c| lattice.index(c))
            .filter(|cell| kind[*cell] == INSIDE);
        match near {
            Some(cell) => source.push(cell),
            None => return Err("there is no air behind the driver on the grid.".into()),
        }
        notes.push("The cone is smaller than one cell, so one cell drives the air.".into());
    }

    let inside = kind.iter().filter(|k| **k == INSIDE).count();
    let mut low = [f64::MAX; 3];
    let mut high = [f64::MIN; 3];
    for (cell, k) in kind.iter().enumerate() {
        if *k == INSIDE {
            let p = lattice.center(lattice.coords(cell));
            for axis in 0..3 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
        }
    }
    let longest = (0..3)
        .map(|axis| (high[axis] - low[axis] + dx).max(0.0))
        .fold(0.0, f64::max);

    // Damping material: each inside cell's flow resistivity (imported damping parts and/or a
    // chamber's `fill` from `catalog/damping/`), and the faces between two inside cells carry
    // the mean of theirs.
    let sigma: Vec<f32> = (0..cells)
        .map(|cell| {
            if kind[cell] != INSIDE {
                return 0.0;
            }
            let p = lattice.center(lattice.coords(cell));
            let from_shape = shape.damping_at(p).unwrap_or(0.0);
            let from_fill = chambers
                .iter()
                .find(|chamber| chamber.fill_sigma.is_some() && chamber.contains(p))
                .and_then(|chamber| chamber.fill_sigma)
                .unwrap_or(0.0);
            from_shape.max(from_fill) as f32
        })
        .collect();
    let mut damping = Vec::new();
    if sigma.iter().any(|s| *s > 0.0) {
        let stride = [n[1] * n[2], n[2], 1];
        for cell in 0..cells {
            let c = lattice.coords(cell);
            for axis in 0..3 {
                if c[axis] == 0 {
                    continue;
                }
                let other = cell - stride[axis];
                if kind[cell] == INSIDE && kind[other] == INSIDE {
                    let face = 0.5 * (sigma[cell] + sigma[other]);
                    if face > 0.0 {
                        damping.push((axis as u8, cell, face));
                    }
                }
            }
        }
        let chamber_fill = chambers.iter().any(|c| c.fill_sigma.is_some());
        notes.push(if chamber_fill {
            "Damping fill in a chamber: one flow resistivity per material at every frequency \
             (a Rayleigh model); the material's own mass and frame are left out. The everyday \
             models use Delany–Bazley Cap from the same resistivity."
                .into()
        } else {
            "Damping material: one flow resistivity per material at every frequency (a Rayleigh \
             model); the material's own mass and frame are left out."
                .into()
        });
    }

    Ok(CabinetGrid {
        grid: Grid {
            n,
            dx,
            origin_m: origin,
            air: kind.iter().map(|k| *k == INSIDE || *k == OUTSIDE).collect(),
            source,
            absorbing,
            openings,
            true_volume_m3: 0.0,
        },
        port_ids: port_indices.iter().map(|i| bores[*i].id.clone()).collect(),
        notes,
        inside_volume_m3: inside as f64 * dx * dx * dx,
        longest_m: longest,
        chamber_air: (0..cells)
            .map(|cell| kind[cell] == INSIDE && duct_of[cell] == NO_DUCT)
            .collect(),
        damping,
    })
}
