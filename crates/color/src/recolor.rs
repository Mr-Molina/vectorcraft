//! Recolor Artwork's colour math (our own): colour identity keys, the White/Black/Grays preserve
//! rules, the methods that map a row of current colours to its new colour, colour reduction by
//! clustering in Lab, the nearest colour of a library and the randomizers.

use std::fmt;

use crate::cms::{Lab, delta_e2000};
use crate::{Color, keep_model};

/// A colour's identity: its model and exact components (to a hundredth of an RGB level, a percent
/// or a Lab unit), so colours that only look alike (a CMYK red and an RGB red) stay apart and keep
/// their model. Written as `rgb 255 128 0` (0–255), `cmyk 0 100 100 0`, `gray 40` (percent) or
/// `lab 55 60 -40` (L 0–100, a and b).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColorKey {
    model: u8,
    v: [i32; 4],
}

/// Key resolution: RGB components are kept to 1/RGB_Q, CMYK and Gray ones to 1/INK_Q and Lab ones
/// to 1/LAB_Q (hundredths of the written units).
const RGB_Q: f32 = 25_500.0;
const INK_Q: f32 = 10_000.0;
const LAB_Q: f32 = 100.0;

impl ColorKey {
    pub fn of(c: &Color) -> Self {
        let rgb = |x: f32| (x.clamp(0.0, 1.0) * RGB_Q).round() as i32;
        let ink = |x: f32| (x.clamp(0.0, 1.0) * INK_Q).round() as i32;
        match *c {
            Color::Rgb { r, g, b } => Self { model: 0, v: [rgb(r), rgb(g), rgb(b), 0] },
            Color::Cmyk { c, m, y, k } => Self { model: 1, v: [ink(c), ink(m), ink(y), ink(k)] },
            Color::Gray { k } => Self { model: 2, v: [ink(k), 0, 0, 0] },
            Color::Lab { l, a, b } => {
                let lab = |x: f32| (x * LAB_Q).round() as i32;
                Self { model: 3, v: [lab(l.clamp(0.0, 100.0)), lab(a), lab(b), 0] }
            }
        }
    }

    /// The colour this key names.
    pub fn color(&self) -> Color {
        let f = |i: usize| self.v[i] as f32 / INK_Q;
        match self.model {
            0 => Color::rgb(self.v[0] as f32 / RGB_Q, self.v[1] as f32 / RGB_Q, self.v[2] as f32 / RGB_Q),
            1 => Color::cmyk(f(0), f(1), f(2), f(3)),
            2 => Color::gray(f(0)),
            _ => Color::lab(self.v[0] as f32 / LAB_Q, self.v[1] as f32 / LAB_Q, self.v[2] as f32 / LAB_Q),
        }
    }

    /// Parse the written form (see [`ColorKey`]); extra spaces and case are ignored.
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split_whitespace();
        let model = it.next()?.to_ascii_lowercase();
        let nums: Vec<f32> = it.map(|t| t.parse::<f32>().ok().filter(|v| v.is_finite())).collect::<Option<_>>()?;
        let c = match (model.as_str(), nums.as_slice()) {
            ("rgb", [r, g, b]) => Color::rgb(r / 255.0, g / 255.0, b / 255.0),
            ("cmyk", [c, m, y, k]) => Color::cmyk(c / 100.0, m / 100.0, y / 100.0, k / 100.0),
            ("gray", [k]) => Color::gray(k / 100.0),
            ("lab", [l, a, b]) => Color::lab(*l, *a, *b),
            _ => return None,
        };
        Some(Self::of(&c))
    }
}

impl fmt::Display for ColorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Components are hundredths of the written units: exact with two decimals.
        let num = |q: i32| {
            let s = format!("{}{}.{:02}", if q < 0 { "-" } else { "" }, q.abs() / 100, q.abs() % 100);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        let [a, b, c, k] = self.v.map(num);
        match self.model {
            0 => write!(f, "rgb {a} {b} {c}"),
            1 => write!(f, "cmyk {a} {b} {c} {k}"),
            2 => write!(f, "gray {a}"),
            _ => write!(f, "lab {a} {b} {c}"),
        }
    }
}

/// The neutral a colour is, if any: paper white, black (rich blacks included) or a grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Neutral {
    White,
    Black,
    Gray,
}

impl Neutral {
    pub fn of(c: &Color) -> Option<Self> {
        const E: f32 = 0.005;
        match *c {
            Color::Gray { k } if k <= E => Some(Neutral::White),
            Color::Gray { k } if k >= 1.0 - E => Some(Neutral::Black),
            Color::Gray { .. } => Some(Neutral::Gray),
            Color::Cmyk { c, m, y, k } if c.max(m).max(y).max(k) <= E => Some(Neutral::White),
            Color::Cmyk { k, .. } if k >= 1.0 - E => Some(Neutral::Black),
            Color::Cmyk { c, m, y, .. } => (c.max(m).max(y) <= E).then_some(Neutral::Gray),
            // Lab: lightness at the ends, no chroma between them (to half a unit).
            Color::Lab { l, a, b } if l >= 99.5 && a.abs().max(b.abs()) <= 0.5 => Some(Neutral::White),
            Color::Lab { l, .. } if l <= 0.5 => Some(Neutral::Black),
            Color::Lab { a, b, .. } => (a.abs().max(b.abs()) <= 0.5).then_some(Neutral::Gray),
            Color::Rgb { r, g, b } => {
                let (lo, hi) = (r.min(g).min(b), r.max(g).max(b));
                if lo >= 1.0 - E {
                    Some(Neutral::White)
                } else if hi <= E {
                    Some(Neutral::Black)
                } else {
                    (hi - lo <= E).then_some(Neutral::Gray)
                }
            }
        }
    }
}

/// Which neutrals Recolor Artwork leaves alone (White and Black by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preserve {
    pub white: bool,
    pub black: bool,
    pub grays: bool,
}

impl Default for Preserve {
    fn default() -> Self {
        Self { white: true, black: true, grays: false }
    }
}

impl Preserve {
    pub const NONE: Self = Self { white: false, black: false, grays: false };

    /// Whether `c` stays as it is.
    pub fn keeps(&self, c: &Color) -> bool {
        match Neutral::of(c) {
            Some(Neutral::White) => self.white,
            Some(Neutral::Black) => self.black,
            Some(Neutral::Gray) => self.grays,
            None => false,
        }
    }
}

/// How the colours of a row take its new colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    /// Every colour becomes the new colour.
    #[default]
    Exact,
    /// Tints of the row's darkest colour become the same tints of the new colour; other colours
    /// become the new colour.
    PreserveTints,
    /// Every colour becomes a tint of the new colour as light, relative to the row's darkest colour,
    /// as it is.
    ScaleTints,
    /// The colour of average lightness becomes the new colour; lighter and darker ones become tints
    /// and shades of it.
    TintsShades,
    /// The row's most saturated colour becomes the new colour; the others turn by the same hue and
    /// keep their saturation and brightness.
    HueShift,
}

impl Method {
    pub const ALL: [Method; 5] = [Method::Exact, Method::PreserveTints, Method::ScaleTints, Method::TintsShades, Method::HueShift];

    pub fn id(self) -> &'static str {
        match self {
            Method::Exact => "exact",
            Method::PreserveTints => "preserveTints",
            Method::ScaleTints => "scaleTints",
            Method::TintsShades => "tintsShades",
            Method::HueShift => "hueShift",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Method::Exact => "Exact",
            Method::PreserveTints => "Preserve Tints",
            Method::ScaleTints => "Scale Tints",
            Method::TintsShades => "Tints and Shades",
            Method::HueShift => "Hue Shift",
        }
    }
    /// A method from its id or label (case and spaces ignored).
    pub fn parse(s: &str) -> Option<Self> {
        let key = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
        let want = key(s);
        Self::ALL.into_iter().find(|m| key(m.id()) == want || key(m.label()) == want)
    }

    /// The colour of `row` the new colour stands for: the most saturated for Hue Shift, the one
    /// of average lightness for Tints and Shades, else the darkest. `None` for an empty row.
    pub fn key_color(self, row: &[Color]) -> Option<Color> {
        let labs: Vec<(Color, Lab)> = row.iter().map(|c| (*c, c.to_lab())).collect();
        let pick = |score: &dyn Fn(&Lab) -> f32| labs.iter().min_by(|a, b| score(&a.1).total_cmp(&score(&b.1))).map(|x| x.0);
        match self {
            Method::HueShift => pick(&|l| -l.chroma() * 1000.0 + l.l),
            Method::TintsShades => {
                let mean = mean_l(&labs);
                pick(&|l| (l.l - mean).abs())
            }
            _ => pick(&|l| l.l * 1000.0 - l.chroma()),
        }
    }
}

fn mean_l(labs: &[(Color, Lab)]) -> f32 {
    labs.iter().map(|x| x.1.l).sum::<f32>() / labs.len().max(1) as f32
}

/// Ink of `c` in `model`'s terms: CMYK and Gray components, 1 − RGB, or Lab's distance from paper
/// white (as [`Color::tinted`] scales it).
fn ink(c: Color, model: crate::cms::Model) -> [f32; 4] {
    match c.in_model(model) {
        Color::Cmyk { c, m, y, k } => [c, m, y, k],
        Color::Gray { k } => [k, 0.0, 0.0, 0.0],
        Color::Rgb { r, g, b } => [1.0 - r, 1.0 - g, 1.0 - b, 0.0],
        Color::Lab { l, a, b } => [(100.0 - l) / 100.0, a / 100.0, b / 100.0, 0.0],
    }
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// How much of `base`'s ink `c` carries (0 = paper, 1 = `base` or darker), measured in `c`'s model:
/// the tint of `base` closest to `c`.
pub fn tint_of(c: Color, base: Color) -> f32 {
    let (a, b) = (ink(c, c.model()), ink(base, c.model()));
    let bb = dot(b, b);
    if bb <= 1e-9 { 1.0 } else { (dot(a, b) / bb).clamp(0.0, 1.0) }
}

/// Whether `c` is a tint of `base` (its ink a fraction of `base`'s).
pub fn is_tint(c: Color, base: Color) -> bool {
    let t = tint_of(c, base);
    let (a, b) = (ink(c, c.model()), ink(base, c.model()));
    a.iter().zip(b).all(|(x, y)| (x - y * t).abs() <= 0.01)
}

/// `c` darkened by `f` (0..1) toward black: more K for CMYK and Gray, scaled RGB and Lab.
pub fn shaded(c: Color, f: f32) -> Color {
    let f = f.clamp(0.0, 1.0);
    match c {
        Color::Cmyk { c, m, y, k } => Color::cmyk(c, m, y, k + (1.0 - k) * f),
        Color::Gray { k } => Color::gray(k + (1.0 - k) * f),
        Color::Lab { l, a, b } => Color::lab(l * (1.0 - f), a * (1.0 - f), b * (1.0 - f)),
        Color::Rgb { r, g, b } => Color::rgb(r * (1.0 - f), g * (1.0 - f), b * (1.0 - f)),
    }
}

/// One row of Recolor Artwork prepared for mapping: its current colours take `to` by `method`.
#[derive(Clone, Copy, Debug)]
pub struct RowMap {
    method: Method,
    to: Color,
    key: Color,
    mean_l: f32,
    /// Hue turn from the key colour to the new colour (Hue Shift).
    turn: f32,
}

impl RowMap {
    /// The map of a row of current colours `row` (non-empty) to the new colour `to`.
    pub fn new(method: Method, row: &[Color], to: Color) -> Self {
        let key = method.key_color(row).unwrap_or(to);
        let mean_l = match method {
            Method::TintsShades => mean_l(&row.iter().map(|c| (*c, c.to_lab())).collect::<Vec<_>>()),
            _ => 0.0,
        };
        Self { method, to, key, mean_l, turn: to.to_hsb()[0] - key.to_hsb()[0] }
    }

    /// The colour of the row the new colour stands for.
    pub fn key(&self) -> Color {
        self.key
    }

    /// The new colour of current colour `c`, in `c`'s model.
    pub fn map(&self, c: Color) -> Color {
        let l = c.to_lab().l;
        if !l.is_finite() {
            return c;
        }
        let to = keep_model(c, self.to);
        match self.method {
            Method::Exact => to,
            Method::PreserveTints if is_tint(c, self.key) => to.tinted(tint_of(c, self.key)),
            Method::PreserveTints => to,
            Method::ScaleTints => to.tinted(tint_of(c, self.key)),
            Method::TintsShades => {
                let d = c.to_lab().l - self.mean_l;
                if d >= 0.0 { to.tinted(1.0 - d / (100.0 - self.mean_l).max(1e-3)) } else { shaded(to, -d / self.mean_l.max(1e-3)) }
            }
            Method::HueShift if c == self.key => to,
            Method::HueShift => {
                let [h, s, v] = c.to_hsb();
                if s <= 0.0 { c } else { keep_model(c, Color::from_hsb(h + self.turn, s, v)) }
            }
        }
    }
}

/// Group `colors` into at most `n` rows of similar colours, for colour reduction: k-means in Lab
/// (lightness counting half, so tints and shades of a hue stay together) weighted by `weights`,
/// seeded with the heaviest colour and then farthest-first, so the result is the same on every
/// run. Returns the rows as indices into `colors`, ordered by their first index (each in input
/// order).
pub fn cluster(colors: &[Color], weights: &[f32], n: usize) -> Vec<Vec<usize>> {
    let pts: Vec<[f32; 3]> = colors
        .iter()
        .map(|c| {
            let l = c.to_lab();
            [l.l * 0.5, l.a, l.b]
        })
        .collect();
    let w = |i: usize| weights.get(i).copied().filter(|v| v.is_finite() && *v > 0.0).unwrap_or(1.0).max(1e-6);
    let d2 = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let n = n.max(1);
    if pts.is_empty() {
        return vec![];
    }
    // Seeds: the heaviest colour, then each time the one farthest from every seed so far.
    let first = (0..pts.len()).fold(0, |best, i| if w(i) > w(best) { i } else { best });
    let mut centres = vec![pts[first]];
    let mut near: Vec<f32> = pts.iter().map(|p| d2(p, &pts[first])).collect();
    while centres.len() < n {
        let (i, far) = near.iter().enumerate().fold((0, 0.0f32), |b, (i, d)| if *d > b.1 { (i, *d) } else { b });
        if far <= 1e-6 {
            break;
        }
        centres.push(pts[i]);
        for (j, p) in pts.iter().enumerate() {
            near[j] = near[j].min(d2(p, &pts[i]));
        }
    }
    let nearest = |p: &[f32; 3], cs: &[[f32; 3]]| (0..cs.len()).min_by(|a, b| d2(p, &cs[*a]).total_cmp(&d2(p, &cs[*b]))).unwrap_or(0);
    let mut of: Vec<usize> = pts.iter().map(|p| nearest(p, &centres)).collect();
    for _ in 0..50 {
        let mut sum = vec![([0.0f32; 3], 0.0f32); centres.len()];
        for (i, p) in pts.iter().enumerate() {
            let s = &mut sum[of[i]];
            for (acc, v) in s.0.iter_mut().zip(p) {
                *acc += v * w(i);
            }
            s.1 += w(i);
        }
        for (c, (s, wt)) in centres.iter_mut().zip(&sum) {
            if *wt > 0.0 {
                *c = s.map(|v| v / wt);
            }
        }
        let next: Vec<usize> = pts.iter().map(|p| nearest(p, &centres)).collect();
        if next == of {
            break;
        }
        of = next;
    }
    let mut rows: Vec<Vec<usize>> = vec![vec![]; centres.len()];
    for (i, c) in of.into_iter().enumerate() {
        rows[c].push(i);
    }
    rows.retain(|r| !r.is_empty());
    rows.sort_by_key(|r| r[0]);
    rows
}

/// A set of colours to snap to (Limit to Library), with their Lab values cached.
#[derive(Clone, Debug, Default)]
pub struct Palette(Vec<(Color, Lab)>);

impl Palette {
    pub fn new(colors: impl IntoIterator<Item = Color>) -> Self {
        Self(colors.into_iter().map(|c| (c, c.to_lab())).collect())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// The palette colour closest to `c` (CIEDE2000), in its own model; `c` for an empty palette.
    pub fn nearest(&self, c: Color) -> Color {
        let l = c.to_lab();
        self.0.iter().min_by(|a, b| delta_e2000(a.1, l).total_cmp(&delta_e2000(b.1, l))).map_or(c, |x| x.0)
    }
}

/// A small deterministic generator (xorshift64*) for the randomizers.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `lo..hi`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next_u64() >> 40) as f32 / (1u64 << 24) as f32)
    }
    /// Shuffle `v` in place (Fisher–Yates).
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
    /// `c` with a random saturation and brightness, its hue kept (greys stay grey), in its model.
    pub fn saturation_brightness(&mut self, c: Color) -> Color {
        let [h, s, _] = c.to_hsb();
        let (s2, v2) = (self.range(0.25, 1.0), self.range(0.3, 1.0));
        keep_model(c, Color::from_hsb(h, if s <= 0.0 { 0.0 } else { s2 }, v2))
    }
}

/// A cluster of similar colours: its representative colour and the objects that have them.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorCluster {
    /// The leader's colour: the colour every member is within the threshold of.
    pub representative: Color,
    /// Member ids, ascending. The first id with the representative's exact colour is the
    /// representative object.
    pub node_ids: Vec<u64>,
    /// Mean CIEDE2000 distance to the representative over the members other than the
    /// representative object (members with its exact colour count as 0); 0 for a single member.
    pub avg_delta_e: f64,
}

/// CIEDE2000's lightness weight S_L at mean lightness `l_mean`. It grows with |l_mean − 50|.
fn s_l(l_mean: f64) -> f64 {
    let d2 = (l_mean - 50.0).powi(2);
    1.0 + 0.015 * d2 / (20.0 + d2).sqrt()
}

/// An exact colour identity for deduplication: the model and the components' bit patterns
/// (−0.0 read as 0.0). Unlike [`ColorKey`] it neither rounds nor clamps (Lab L may leave 0..100).
#[derive(PartialEq, Eq, Hash)]
struct ExactKey(u8, [u32; 4]);

impl ExactKey {
    fn of(c: &Color) -> Self {
        let b = |x: f32| if x == 0.0 { 0 } else { x.to_bits() };
        match *c {
            Color::Rgb { r, g, b: bl } => Self(0, [b(r), b(g), b(bl), 0]),
            Color::Cmyk { c, m, y, k } => Self(1, [b(c), b(m), b(y), b(k)]),
            Color::Gray { k } => Self(2, [b(k), 0, 0, 0]),
            Color::Lab { l, a, b: bb } => Self(3, [b(l), b(a), b(bb), 0]),
        }
    }
}

/// Leader clustering of `(id, colour)` pairs by CIEDE2000 distance (kL = kC = kH = 1).
///
/// Exact duplicates (same model and components) are merged first and converted to Lab once. The
/// unique colours are then visited in a total order (L, a, b by `total_cmp`, then smallest id),
/// so the result does not depend on the order of `items`. Each colour joins the nearest existing
/// leader within `max_delta_e` (the earliest on ties) or becomes a new leader.
///
/// This is *leader* clustering, not complete-linkage: every member is within `max_delta_e` of
/// its leader, but two members of one cluster can be up to about `2 · max_delta_e` apart.
/// `max_delta_e` 0 groups exact matches only. A negative or non-finite threshold gives no
/// clusters; colours whose Lab is not finite each stay alone.
pub fn cluster_colors_by_delta_e(items: &[(u64, Color)], max_delta_e: f64) -> Vec<ColorCluster> {
    cluster_impl(items, max_delta_e, true)
}

/// [`cluster_colors_by_delta_e`]; `prune` skips leaders whose lightness alone puts them out of
/// reach (the result is the same either way; tests compare the two).
fn cluster_impl(items: &[(u64, Color)], max_delta_e: f64, prune: bool) -> Vec<ColorCluster> {
    if items.is_empty() || !max_delta_e.is_finite() || max_delta_e < 0.0 {
        return Vec::new();
    }
    // Exact duplicates: colour → (colour of the smallest id, ids).
    let mut unique: std::collections::HashMap<ExactKey, (u64, Color, Vec<u64>)> = std::collections::HashMap::new();
    for &(id, c) in items {
        let e = unique.entry(ExactKey::of(&c)).or_insert((id, c, Vec::new()));
        if id < e.0 {
            (e.0, e.1) = (id, c);
        }
        e.2.push(id);
    }
    let mut unique: Vec<(Lab, u64, Color, Vec<u64>)> = unique
        .into_values()
        .map(|(min_id, c, mut ids)| {
            ids.sort_unstable();
            ids.dedup();
            (c.to_lab(), min_id, c, ids)
        })
        .collect();
    unique.sort_by(|x, y| x.0.l.total_cmp(&y.0.l).then(x.0.a.total_cmp(&y.0.a)).then(x.0.b.total_cmp(&y.0.b)).then(x.1.cmp(&y.1)));

    let finite = |l: &Lab| l.l.is_finite() && l.a.is_finite() && l.b.is_finite();
    // ΔE00 ≥ |ΔL| / S_L(L̄), and L̄ of any two colours lies between the smallest and largest L,
    // so S_L at the extreme farthest from 50 bounds every pair (any finite L, not only 0..100).
    // The small factor absorbs rounding.
    let far = unique.iter().filter(|u| finite(&u.0)).map(|u| (u.0.l as f64 - 50.0).abs()).fold(0.0, f64::max);
    let reach_span = s_l(50.0 + far) * 1.0001 * max_delta_e;
    // Leaders with finite Lab, in creation order. Unique colours arrive sorted by L, so their L
    // values ascend and a binary search skips the ones too dark to be within reach.
    let mut leaders: Vec<(Lab, usize)> = Vec::new();
    // Per cluster: (representative colour, ids, sum of member ΔE, number of ΔE terms).
    let mut out: Vec<(Color, Vec<u64>, f64, usize)> = Vec::new();
    for (lab, _, color, ids) in unique {
        let mut best: Option<(usize, f64)> = None;
        if finite(&lab) {
            let reach = lab.l as f64 - reach_span;
            let start = if prune && reach.is_finite() { leaders.partition_point(|(r, _)| (r.l as f64) < reach) } else { 0 };
            for (rep, idx) in leaders.get(start..).unwrap_or_default() {
                let de = delta_e2000(*rep, lab) as f64;
                if de <= max_delta_e && best.is_none_or(|(_, b)| de < b) {
                    best = Some((*idx, de));
                }
            }
        }
        match best.and_then(|(idx, de)| out.get_mut(idx).map(|c| (c, de))) {
            Some((cl, de)) => {
                cl.2 += de * ids.len() as f64;
                cl.3 += ids.len();
                cl.1.extend(ids);
            }
            None => {
                if finite(&lab) {
                    leaders.push((lab, out.len()));
                }
                // The representative object itself contributes no ΔE term; its exact twins add 0.
                let terms = ids.len().saturating_sub(1);
                out.push((color, ids, 0.0, terms));
            }
        }
    }
    out.into_iter()
        .map(|(representative, mut node_ids, sum, terms)| {
            node_ids.sort_unstable();
            ColorCluster { representative, node_ids, avg_delta_e: if terms == 0 { 0.0 } else { sum / terms as f64 } }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Color, b: Color) -> bool {
        let v = |c: Color| match c {
            Color::Cmyk { c, m, y, k } => vec![c, m, y, k],
            Color::Rgb { r, g, b } => vec![r, g, b],
            Color::Gray { k } => vec![k],
            Color::Lab { l, a, b } => vec![l / 100.0, a / 100.0, b / 100.0],
        };
        a.model() == b.model() && v(a).iter().zip(v(b)).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn keys_keep_the_model_and_round_trip() {
        let colors = [
            Color::rgb(1.0, 0.5, 0.0),
            Color::rgb(1.0 / 3.0, 0.2, 0.9),
            Color::cmyk(0.0, 1.0, 1.0, 0.0),
            Color::cmyk(0.1234, 0.5, 0.0, 0.07),
            Color::gray(0.4),
        ];
        for c in colors {
            let k = ColorKey::of(&c);
            assert_eq!(ColorKey::parse(&k.to_string()), Some(k), "{k}");
            assert!(close(k.color(), c));
        }
        assert_eq!(ColorKey::of(&Color::rgb(1.0, 0.5, 0.0)).to_string(), "rgb 255 127.5 0");
        assert_eq!(ColorKey::of(&Color::cmyk(0.0, 1.0, 1.0, 0.0)).to_string(), "cmyk 0 100 100 0");
        assert_eq!(ColorKey::of(&Color::gray(0.4)).to_string(), "gray 40");
        assert_eq!(ColorKey::of(&Color::rgb8(18, 52, 86)).to_string(), "rgb 18 52 86", "8-bit colours read as whole levels");
        // A CMYK red and an RGB red are different colours.
        assert_ne!(ColorKey::of(&Color::cmyk(0.0, 1.0, 1.0, 0.0)), ColorKey::of(&Color::rgb(1.0, 0.0, 0.0)));
        assert_eq!(ColorKey::parse(" CMYK 0 100  100 0 "), ColorKey::parse("cmyk 0 100 100 0"));
        let lab = Color::lab(55.0, 60.5, -40.25);
        assert_eq!(ColorKey::of(&lab).to_string(), "lab 55 60.5 -40.25");
        assert_eq!(ColorKey::parse("lab 55 60.5 -40.25").map(|k| k.color()), Some(lab));
        assert_ne!(ColorKey::of(&lab), ColorKey::of(&keep_model(Color::WHITE, lab)), "a Lab colour isn't its RGB lookalike");
        assert_eq!(ColorKey::parse("#ff0000"), None);
        assert_eq!(ColorKey::parse("rgb 1 2"), None);
    }

    #[test]
    fn neutrals_and_preserve() {
        assert_eq!(Neutral::of(&Color::WHITE), Some(Neutral::White));
        assert_eq!(Neutral::of(&Color::cmyk(0.0, 0.0, 0.0, 0.0)), Some(Neutral::White));
        assert_eq!(Neutral::of(&Color::cmyk(0.6, 0.4, 0.4, 1.0)), Some(Neutral::Black), "rich black");
        assert_eq!(Neutral::of(&Color::gray(0.5)), Some(Neutral::Gray));
        assert_eq!(Neutral::of(&Color::rgb(0.5, 0.5, 0.5)), Some(Neutral::Gray));
        assert_eq!(Neutral::of(&Color::cmyk(0.0, 0.0, 0.0, 0.3)), Some(Neutral::Gray));
        assert_eq!(Neutral::of(&Color::rgb(0.5, 0.4, 0.5)), None);
        assert_eq!(Neutral::of(&Color::lab(100.0, 0.0, 0.0)), Some(Neutral::White));
        assert_eq!(Neutral::of(&Color::lab(0.0, 0.0, 0.0)), Some(Neutral::Black));
        assert_eq!(Neutral::of(&Color::lab(50.0, 0.2, -0.3)), Some(Neutral::Gray));
        assert_eq!(Neutral::of(&Color::lab(50.0, 20.0, 0.0)), None);
        let p = Preserve::default();
        assert!(p.keeps(&Color::BLACK) && p.keeps(&Color::WHITE) && !p.keeps(&Color::gray(0.5)));
        assert!(!Preserve::NONE.keeps(&Color::BLACK));
    }

    #[test]
    fn methods_parse_by_id_and_label() {
        for m in Method::ALL {
            assert_eq!(Method::parse(m.id()), Some(m));
            assert_eq!(Method::parse(m.label()), Some(m));
        }
        assert_eq!(Method::parse("tints & shades"), Some(Method::TintsShades));
        assert_eq!(Method::parse("nope"), None);
    }

    #[test]
    fn scale_tints_keep_tint_ratios() {
        let red = Color::cmyk(0.0, 1.0, 1.0, 0.0);
        let pink = red.tinted(0.4);
        let row = [pink, red];
        let m = RowMap::new(Method::ScaleTints, &row, Color::cmyk(1.0, 0.5, 0.0, 0.0));
        assert!(close(m.key(), red), "the darkest colour is the key");
        assert!(close(m.map(red), Color::cmyk(1.0, 0.5, 0.0, 0.0)));
        assert!(close(m.map(pink), Color::cmyk(0.4, 0.2, 0.0, 0.0)), "{:?}", m.map(pink));
        // RGB tints mix toward white.
        let blue = Color::rgb(0.0, 0.0, 1.0);
        let m = RowMap::new(Method::ScaleTints, &[blue, blue.tinted(0.5)], Color::rgb(1.0, 0.0, 0.0));
        assert!(close(m.map(blue.tinted(0.5)), Color::rgb(1.0, 0.5, 0.5)));
        // Lab tints too (toward paper white).
        let base = Color::lab(40.0, 60.0, -20.0);
        let to = Color::lab(50.0, -40.0, 30.0);
        let m = RowMap::new(Method::ScaleTints, &[base, base.tinted(0.25)], to);
        assert!(close(m.map(base.tinted(0.25)), to.tinted(0.25)), "{:?}", m.map(base.tinted(0.25)));
    }

    #[test]
    fn preserve_tints_only_scales_real_tints() {
        let red = Color::cmyk(0.0, 1.0, 1.0, 0.0);
        let other = Color::cmyk(0.0, 0.3, 0.8, 0.0);
        let to = Color::cmyk(1.0, 0.0, 0.0, 0.0);
        let m = RowMap::new(Method::PreserveTints, &[red, red.tinted(0.5), other], to);
        assert!(close(m.map(red.tinted(0.5)), Color::cmyk(0.5, 0.0, 0.0, 0.0)));
        assert!(close(m.map(other), to), "not a tint: exact");
        let s = RowMap::new(Method::ScaleTints, &[red, other], to);
        assert!(!close(s.map(other), to), "Scale Tints scales every colour");
    }

    #[test]
    fn exact_tints_shades_and_hue_shift() {
        let to = Color::rgb(0.0, 0.4, 1.0);
        let row = [Color::rgb(0.9, 0.2, 0.2), Color::rgb(0.5, 0.1, 0.1), Color::rgb(1.0, 0.7, 0.7)];
        let e = RowMap::new(Method::Exact, &row, to);
        assert!(row.iter().all(|c| e.map(*c) == to));
        let ts = RowMap::new(Method::TintsShades, &row, to);
        let l = |c: Color| c.to_lab().l;
        assert!(l(ts.map(row[2])) > l(ts.map(row[0])) && l(ts.map(row[0])) > l(ts.map(row[1])), "lighter stays lighter");
        let hs = RowMap::new(Method::HueShift, &row, to);
        assert_eq!(hs.map(hs.key()), to);
        let [h, s, v] = hs.map(row[1]).to_hsb();
        let [_, s0, v0] = row[1].to_hsb();
        assert!((h - to.to_hsb()[0]).abs() < 1.0 && (s - s0).abs() < 1e-3 && (v - v0).abs() < 1e-3, "{h} {s} {v}");
        // Results keep the model of the colour they replace.
        let m = RowMap::new(Method::Exact, &[Color::gray(0.3)], to);
        assert!(matches!(m.map(Color::gray(0.3)), Color::Gray { .. }));
        let lab = [Color::lab(40.0, 50.0, 30.0), Color::lab(70.0, 20.0, 10.0)];
        for method in Method::ALL {
            let m = RowMap::new(method, &lab, to);
            assert!(lab.iter().all(|c| matches!(m.map(*c), Color::Lab { .. })), "{}", method.label());
        }
        assert!(matches!(shaded(lab[0], 0.5), Color::Lab { .. }));
    }

    #[test]
    fn clustering_reduces_to_n_rows_of_similar_colours() {
        let colors =
            [Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 0.0, 1.0), Color::rgb(0.9, 0.1, 0.05), Color::rgb(0.1, 0.1, 0.9), Color::rgb(1.0, 0.2, 0.2)];
        let rows = cluster(&colors, &[5.0, 4.0, 1.0, 1.0, 1.0], 2);
        assert_eq!(rows, vec![vec![0, 2, 4], vec![1, 3]]);
        assert_eq!(cluster(&colors, &[1.0; 5], 9).len(), 5, "never more rows than colours");
        assert_eq!(cluster(&colors, &[1.0; 5], 1), vec![vec![0, 1, 2, 3, 4]]);
        assert_eq!(cluster(&[Color::WHITE, Color::WHITE], &[1.0, 1.0], 2).len(), 1, "identical colours share a row");
        assert!(cluster(&[], &[], 3).is_empty());
    }

    #[test]
    fn palette_snaps_to_the_nearest_colour() {
        let p = Palette::new([Color::rgb(1.0, 0.0, 0.0), Color::cmyk(1.0, 0.0, 0.0, 0.0), Color::gray(0.5)]);
        assert_eq!(p.nearest(Color::rgb(0.9, 0.1, 0.1)), Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(p.nearest(Color::rgb(0.45, 0.45, 0.45)), Color::gray(0.5));
        let lab = Palette::new([Color::lab(50.0, 70.0, 50.0), Color::lab(50.0, -60.0, 50.0)]);
        assert_eq!(lab.nearest(Color::rgb(0.9, 0.1, 0.1)), Color::lab(50.0, 70.0, 50.0), "library colours keep their model");
        assert_eq!(Palette::default().nearest(Color::WHITE), Color::WHITE);
    }

    #[test]
    fn randomizers_are_deterministic_and_keep_hue() {
        let mut a: Vec<u32> = (0..10).collect();
        let mut b = a.clone();
        Rng::new(7).shuffle(&mut a);
        Rng::new(7).shuffle(&mut b);
        assert_eq!(a, b);
        assert_ne!(a, (0..10).collect::<Vec<_>>());
        let c = Color::cmyk(0.0, 0.8, 0.9, 0.1);
        let r = Rng::new(3).saturation_brightness(c);
        assert!(matches!(r, Color::Cmyk { .. }));
        assert!(matches!(Rng::new(3).saturation_brightness(Color::lab(50.0, 40.0, 20.0)), Color::Lab { .. }));
        assert_eq!(Rng::new(3).saturation_brightness(Color::gray(0.4)).to_hsb()[1], 0.0, "greys stay grey");
    }

    #[test]
    fn test_cluster_colors_by_delta_e_threshold() {
        let red1 = (1, Color::rgb(0.9, 0.1, 0.1));
        let red2 = (2, Color::rgb(0.91, 0.11, 0.1));
        let blue = (3, Color::rgb(0.1, 0.1, 0.9));
        let clusters = cluster_colors_by_delta_e(&[red1, red2, blue], 5.0);
        assert_eq!(clusters.len(), 2);
        let ids: Vec<Vec<u64>> = clusters.iter().map(|c| c.node_ids.clone()).collect();
        assert!(ids.contains(&vec![1, 2]) && ids.contains(&vec![3]), "{ids:?}");
    }

    #[test]
    fn test_cluster_colors_rejects_non_finite_and_negative() {
        let red = (1, Color::rgb(0.9, 0.1, 0.1));
        assert!(cluster_colors_by_delta_e(&[red], f64::NAN).is_empty());
        assert!(cluster_colors_by_delta_e(&[red], f64::INFINITY).is_empty());
        assert!(cluster_colors_by_delta_e(&[red], -1.0).is_empty());
        assert!(cluster_colors_by_delta_e(&[], 5.0).is_empty());
    }

    #[test]
    fn cluster_threshold_zero_is_exact_match() {
        let a = Color::rgb(0.5, 0.2, 0.1);
        let items = [(1, a), (2, Color::rgb(0.501, 0.2, 0.1)), (3, a)];
        let c = cluster_colors_by_delta_e(&items, 0.0);
        let ids: Vec<Vec<u64>> = c.iter().map(|c| c.node_ids.clone()).collect();
        assert_eq!(c.len(), 2, "{ids:?}");
        assert!(ids.contains(&vec![1, 3]) && ids.contains(&vec![2]));
    }

    #[test]
    fn cluster_is_independent_of_input_order() {
        // A chain of reds, each about 2–3 ΔE from the next: leader clustering is order sensitive
        // unless the colours are visited in a fixed order.
        let items: Vec<(u64, Color)> = (0..12).map(|i| (i as u64 + 10, Color::rgb(0.95 - i as f32 * 0.02, 0.1, 0.1))).collect();
        let norm = |mut v: Vec<ColorCluster>| {
            v.sort_by_key(|c| c.node_ids.first().copied());
            v
        };
        let base = norm(cluster_colors_by_delta_e(&items, 4.0));
        assert!(base.len() > 1, "the chain spans more than one cluster");
        let mut rng = Rng::new(11);
        for _ in 0..20 {
            let mut p = items.clone();
            rng.shuffle(&mut p);
            assert_eq!(norm(cluster_colors_by_delta_e(&p, 4.0)), base);
        }
        let mut rev = items.clone();
        rev.reverse();
        assert_eq!(norm(cluster_colors_by_delta_e(&rev, 4.0)), base);
    }

    #[test]
    fn cluster_average_excludes_the_representative() {
        let a = Color::rgb(0.9, 0.1, 0.1);
        let b = Color::rgb(0.91, 0.11, 0.1);
        let single = cluster_colors_by_delta_e(&[(1, a)], 5.0);
        assert_eq!(single[0].avg_delta_e, 0.0);
        let twins = cluster_colors_by_delta_e(&[(1, a), (2, a)], 5.0);
        assert_eq!((twins.len(), twins[0].avg_delta_e), (1, 0.0));
        let pair = cluster_colors_by_delta_e(&[(1, a), (2, b)], 5.0);
        assert_eq!(pair.len(), 1);
        let de = delta_e2000(pair[0].representative.to_lab(), if pair[0].representative == a { b } else { a }.to_lab()) as f64;
        assert!((pair[0].avg_delta_e - de).abs() < 1e-9, "{} vs {de}", pair[0].avg_delta_e);
        assert!(de > 0.0);
    }

    #[test]
    fn cluster_handles_many_distinct_colours() {
        // 4096 distinct colours: pruning by lightness keeps this quick and the result complete.
        let items: Vec<(u64, Color)> =
            (0..4096u64).map(|i| (i, Color::rgb8((i % 16 * 16) as u8, (i / 16 % 16 * 16) as u8, (i / 256 * 16) as u8))).collect();
        let c = cluster_colors_by_delta_e(&items, 3.0);
        let total: usize = c.iter().map(|c| c.node_ids.len()).sum();
        assert_eq!(total, items.len());
        for cl in &c {
            let rep = cl.representative.to_lab();
            for id in &cl.node_ids {
                let col = items.iter().find(|x| x.0 == *id).map(|x| x.1).unwrap();
                assert!(delta_e2000(rep, col.to_lab()) as f64 <= 3.0);
            }
        }
    }

    #[test]
    fn pruning_holds_for_lightness_outside_0_100() {
        // S_L(104) ≈ 1.807 > 1.8: a fixed 1.8 bound would have pruned the L = 99 leader.
        let items = [(1, Color::lab(99.0, 0.0, 0.0)), (2, Color::lab(109.0, 0.0, 0.0))];
        let de = delta_e2000(Lab::new(99.0, 0.0, 0.0), Lab::new(109.0, 0.0, 0.0)) as f64;
        assert!(de < 5.54 && 10.0 / de > 1.8, "{de}");
        let pruned = cluster_impl(&items, 5.54, true);
        assert_eq!(pruned, cluster_impl(&items, 5.54, false));
        assert_eq!(pruned.len(), 1, "within the threshold: one cluster");
    }

    #[test]
    fn pruned_matches_naive_on_random_lab() {
        let mut rng = Rng::new(2024);
        for round in 0..60 {
            let n = 20 + round * 3;
            let items: Vec<(u64, Color)> = (0..n as u64)
                .map(|i| {
                    let (l, a, b) = (rng.range(-20.0, 130.0), rng.range(-40.0, 40.0), rng.range(-40.0, 40.0));
                    // Some exact repeats too.
                    if i % 7 == 3 { (i, Color::lab(l.round(), 0.0, 0.0)) } else { (i, Color::lab(l, a, b)) }
                })
                .collect();
            for t in [0.0, 1.0, 3.5, 8.0, 25.0] {
                assert_eq!(cluster_impl(&items, t, true), cluster_impl(&items, t, false), "round {round} t {t}");
            }
        }
    }

    #[test]
    fn exact_dedupe_does_not_clamp_or_round() {
        let items = [(1, Color::lab(105.0, 0.0, 0.0)), (2, Color::lab(130.0, 0.0, 0.0))];
        assert_eq!(cluster_colors_by_delta_e(&items, 0.0).len(), 2, "L 105 and 130 are different colours");
        let items = [(1, Color::lab(-5.0, 0.0, 0.0)), (2, Color::lab(-10.0, 0.0, 0.0))];
        assert_eq!(cluster_colors_by_delta_e(&items, 0.0).len(), 2);
        // −0.0 and 0.0 are the same colour.
        let items = [(1, Color::lab(50.0, -0.0, 0.0)), (2, Color::lab(50.0, 0.0, 0.0))];
        assert_eq!(cluster_colors_by_delta_e(&items, 0.0).len(), 1);
        assert!(ExactKey::of(&Color::rgb(0.0, 0.5, 1.0)) == ExactKey::of(&Color::rgb(-0.0, 0.5, 1.0)));
        assert!(ExactKey::of(&Color::rgb(0.5, 0.5, 0.5)) != ExactKey::of(&Color::gray(0.5)));
    }

    #[test]
    fn test_cluster_weights_sanitized() {
        let colors = [Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 1.0, 0.0)];
        let weights = [f32::NAN, -5.0];
        let res = cluster(&colors, &weights, 2);
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn test_row_map_nan_lightness() {
        let rm = RowMap::new(Method::Exact, &[Color::rgb(1.0, 0.0, 0.0)], Color::rgb(0.0, 0.0, 1.0));
        let nan_color = Color::lab(f32::NAN, 0.0, 0.0);
        let out = rm.map(nan_color);
        assert!(out.to_lab().l.is_nan());
    }
}
