//! Bounded, sample-driven graph state. No animation subscription or GPU filters.
use crate::config::Settings;
use cosmic::iced::widget::svg::Handle;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    time::Instant,
};

const MAX_POINTS: usize = 121;
const MAX_SECONDS: u64 = 120;

#[derive(Clone, Copy, Debug)]
struct Peak {
    value: f32,
    at: Instant,
}
#[derive(Clone, Copy, Debug)]
struct CpuPoint {
    at: Instant,
    average: f32,
    busiest: f32,
}
#[derive(Clone)]
pub struct Image {
    pub handle: Handle,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, PartialEq)]
struct Style {
    height: f32,
    vertical: bool,
    cpu: bool,
    warning: u32,
    critical: u32,
    glow: bool,
    glow_seconds: u32,
    history_seconds: u32,
    history_width: u32,
}
impl Style {
    fn new(settings: &Settings, height: f32, vertical: bool, cpu: bool) -> Self {
        Self {
            height,
            vertical,
            cpu,
            warning: settings.warning,
            critical: settings.critical,
            glow: settings.peak_glow,
            glow_seconds: settings.glow_seconds.max(settings.interval * 2),
            history_seconds: settings.history_seconds,
            history_width: settings.history_width,
        }
    }
}
#[derive(Default)]
pub struct Graphs {
    peaks: BTreeMap<String, Vec<Option<Peak>>>,
    cpu: VecDeque<CpuPoint>,
    core_count: usize,
    now: Option<Instant>,
    // One entry per visible resource, replaced on resize instead of accumulating sizes.
    cache: RefCell<BTreeMap<String, (Style, Image)>>,
}
fn glow_seconds(settings: &Settings) -> f32 {
    // Even slow polling gets one visible falling sample, without extra wakeups.
    settings.glow_seconds.max(settings.interval * 2) as f32
}
fn clean(value: Option<f32>) -> Option<f32> {
    value.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 100.0))
}
impl Graphs {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn clear_peaks(&mut self) {
        self.peaks.clear();
        self.cache.get_mut().clear();
    }
    pub fn observe(
        &mut self,
        cpus: &[f32],
        meters: impl IntoIterator<Item = (String, Vec<Option<f32>>)>,
        now: Instant,
        settings: &Settings,
    ) {
        if self.now.is_some_and(|previous| {
            now.saturating_duration_since(previous)
                > std::time::Duration::from_secs(u64::from(settings.interval) * 3)
        }) {
            self.cpu.clear();
            self.peaks.clear();
        }
        self.now = Some(now);
        self.cache.get_mut().clear();
        if self.core_count != cpus.len() {
            self.cpu.clear();
            self.peaks.remove("cpu");
            self.core_count = cpus.len();
        }
        if !cpus.is_empty() && cpus.iter().all(|v| v.is_finite()) {
            let average = cpus.iter().map(|v| v.clamp(0.0, 100.0)).sum::<f32>() / cpus.len() as f32;
            let busiest = cpus.iter().copied().fold(0.0, f32::max).clamp(0.0, 100.0);
            self.cpu.push_back(CpuPoint {
                at: now,
                average,
                busiest,
            });
        } else {
            self.cpu.clear();
        }
        while self.cpu.len() > MAX_POINTS
            || self.cpu.front().is_some_and(|p| {
                now.saturating_duration_since(p.at).as_secs_f32() > MAX_SECONDS as f32
            })
        {
            self.cpu.pop_front();
        }
        let mut next = BTreeMap::new();
        for (key, values) in meters {
            if !settings.peak_glow {
                continue;
            }
            let old = self.peaks.remove(&key).unwrap_or_default();
            let peaks = values
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    let value = clean(v)?;
                    match old.get(i).copied().flatten() {
                        Some(p)
                            if value < p.value
                                && now.saturating_duration_since(p.at).as_secs_f32()
                                    < glow_seconds(settings) =>
                        {
                            Some(p)
                        }
                        _ => Some(Peak { value, at: now }),
                    }
                })
                .collect();
            next.insert(key, peaks);
        }
        self.peaks = next; // Removed devices cannot leave trails behind.
    }
    fn cached(&self, key: &str, style: Style, make: impl FnOnce() -> (String, f32, f32)) -> Image {
        if let Some((previous, image)) = self.cache.borrow().get(key)
            && previous == &style
        {
            return image.clone();
        }
        let (svg, width, height) = make();
        let image = Image {
            handle: Handle::from_memory(svg.into_bytes()),
            width,
            height,
        };
        self.cache
            .borrow_mut()
            .insert(key.into(), (style, image.clone()));
        image
    }
    pub fn meter(
        &self,
        key: &str,
        values: &[Option<f32>],
        settings: &Settings,
        height: f32,
        vertical: bool,
    ) -> Image {
        let style = Style::new(settings, height, vertical, key == "cpu");
        self.cached(key, style, || {
            self.meter_svg(key, values, settings, height, vertical)
        })
    }
    fn meter_svg(
        &self,
        key: &str,
        values: &[Option<f32>],
        settings: &Settings,
        height: f32,
        vertical: bool,
    ) -> (String, f32, f32) {
        let count = values.iter().filter(|v| clean(**v).is_some()).count();
        let (bar, gap, columns, width, total_height) =
            geometry(count, height, vertical, key == "cpu");
        let mut svg = start(width, total_height);
        for (position, (i, value)) in values
            .iter()
            .enumerate()
            .filter_map(|(i, value)| clean(*value).map(|v| (i, v)))
            .enumerate()
        {
            let x = (position % columns) as f32 * (bar + gap);
            let top = (position / columns) as f32 * (height + 2.0);
            let color = settings.color(Some(value));
            svg.push_str(&format!(r#"<rect x="{x}" y="{top}" width="{bar}" height="{height}" rx="1" fill="{color}" opacity="0.20"/>"#));
            let fill = (value * height / 100.0).max(1.0);
            if settings.peak_glow
                && let Some(p) = self
                    .peaks
                    .get(key)
                    .and_then(|p| p.get(i))
                    .copied()
                    .flatten()
                && let Some(now) = self.now
            {
                let fade = (1.0
                    - now.saturating_duration_since(p.at).as_secs_f32() / glow_seconds(settings))
                .clamp(0.0, 1.0);
                let peak_fill = p.value * height / 100.0;
                if peak_fill > fill && fade > 0.0 {
                    let peak_color = settings.color(Some(p.value));
                    let y = top + height - peak_fill;
                    svg.push_str(&format!(r#"<rect x="{x}" y="{y}" width="{bar}" height="{}" fill="{peak_color}" opacity="{:.3}"/><path d="M{x} {y}h{bar}" stroke="{peak_color}" stroke-width="1" opacity="{:.3}"/>"#,peak_fill-fill,0.22*fade,0.8*fade));
                }
            }
            svg.push_str(&format!(
                r#"<rect x="{x}" y="{}" width="{bar}" height="{fill}" rx="1" fill="{color}"/>"#,
                top + height - fill
            ));
        }
        svg.push_str("</svg>");
        (svg, width, total_height)
    }
    pub fn history(&self, settings: &Settings, height: f32, vertical: bool) -> Image {
        self.cached(
            "cpu-history",
            Style::new(settings, height, vertical, true),
            || self.history_svg(settings, height, vertical),
        )
    }
    fn history_svg(&self, settings: &Settings, height: f32, vertical: bool) -> (String, f32, f32) {
        let width = if vertical {
            height
        } else {
            settings.history_width as f32
        };
        let height = if vertical { height * 2.0 } else { height };
        let mut svg = start(width, height);
        svg.push_str(&format!(
            r##"<rect width="{width}" height="{height}" rx="2" fill="#8b919c" opacity="0.12"/>"##
        ));
        if let Some(now) = self.now {
            let points: Vec<_> = self
                .cpu
                .iter()
                .filter_map(|p| {
                    let age = now.saturating_duration_since(p.at).as_secs_f32();
                    (age <= settings.history_seconds as f32).then_some((
                        1.0 + (width - 2.0) * (1.0 - age / settings.history_seconds as f32),
                        p,
                    ))
                })
                .collect();
            let y = |v: f32| height - 1.0 - (height - 2.0) * v / 100.0;
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                let fill_color = settings.color(Some(last.1.average));
                let path = points
                    .iter()
                    .map(|(x, p)| format!("L{x:.2} {:.2}", y(p.average)))
                    .collect::<String>();
                svg.push_str(&format!(
                    r##"<path d="M{:.2} {}{path}L{:.2} {}Z" fill="{fill_color}" opacity="0.22"/>"##,
                    first.0,
                    height - 1.0,
                    last.0,
                    height - 1.0
                ));
                // At most three paths per trace: batches by severity instead of one widget per point.
                for busiest in [false, true] {
                    for color in ["#65d69a", "#f4ce64", "#f36675"] {
                        let mut path = String::new();
                        let mut connected = false;
                        for pair in points.windows(2) {
                            let (x0, p0) = pair[0];
                            let (x1, p1) = pair[1];
                            let (v0, v1) = if busiest {
                                (p0.busiest, p1.busiest)
                            } else {
                                (p0.average, p1.average)
                            };
                            if settings.color(Some(v1)) == color {
                                if !connected {
                                    path.push_str(&format!("M{x0:.2} {:.2}", y(v0)));
                                }
                                path.push_str(&format!("L{x1:.2} {:.2}", y(v1)));
                                connected = true;
                            } else {
                                connected = false;
                            }
                        }
                        if !path.is_empty() {
                            let attrs = if busiest {
                                r#"stroke-width="1" stroke-dasharray="2 1""#
                            } else {
                                r#"stroke-width="1.5""#
                            };
                            svg.push_str(&format!(
                                r#"<path d="{path}" fill="none" stroke="{color}" {attrs}/>"#
                            ));
                        }
                    }
                }
                // The endpoint remains visible even during first-sample warmup or one-core saturation.
                svg.push_str(&format!(
                    r#"<circle cx="{:.2}" cy="{:.2}" r="1" fill="{}"/>"#,
                    last.0,
                    y(last.1.busiest),
                    settings.color(Some(last.1.busiest))
                ));
            }
        }
        svg.push_str("</svg>");
        (svg, width, height)
    }
}
fn start(width: f32, height: f32) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )
}
fn geometry(count: usize, height: f32, vertical: bool, cpu: bool) -> (f32, f32, usize, f32, f32) {
    let bar = ((if cpu { 4.0 } else { 7.0 }) * height / 24.0)
        .round()
        .max(2.0);
    let gap = if cpu {
        1.0
    } else {
        (2.0 * height / 24.0).round().max(1.0)
    };
    let count = count.max(1);
    let columns = if vertical {
        ((height + gap) / (bar + gap)).floor().max(1.0) as usize
    } else {
        count
    }
    .min(count);
    let width = columns as f32 * (bar + gap) - gap;
    let rows = count.div_ceil(columns);
    (bar, gap, columns, width, rows as f32 * (height + 2.0) - 2.0)
}

#[cfg(test)]
mod tests;
