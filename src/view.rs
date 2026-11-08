use crate::{
    App, Message,
    config::CpuGraph,
    metrics::{bytes, percent},
};
use cosmic::{
    iced::{
        Length,
        alignment::{Horizontal, Vertical},
        widget::svg::Handle,
    },
    prelude::*,
    widget,
};

/// One resource and its panel meters and textual reading.
struct Group {
    key: String,
    label: String,
    icon: &'static str,
    values: Vec<Option<f32>>,
    detail: String,
}
fn capacity(pair: (u64, u64)) -> String {
    if pair.1 == 0 {
        "Unavailable / disabled".into()
    } else {
        format!(
            "{} / {} ({:.0}%)",
            bytes(pair.0 as f64),
            bytes(pair.1 as f64),
            percent(pair.0, pair.1).unwrap_or_default()
        )
    }
}
impl App {
    fn groups(&self) -> Vec<Group> {
        let s = self.snapshot.as_deref();
        let mut groups = Vec::new();
        let definitions = [
            ("CPU", "processor-symbolic"),
            ("RAM", "memory-symbolic"),
            ("Swap", "swap-symbolic"),
            ("Network ↓ ↑", "network-wired-symbolic"),
            ("Disk · space / R / W", "drive-harddisk-symbolic"),
            ("GPU", "video-display-symbolic"),
        ];
        for (index, (label, icon)) in definitions.into_iter().enumerate() {
            if !self.settings.visible[index] {
                continue;
            }
            let (values, detail) = match (index, s) {
                (0, Some(s)) => (
                    s.cpus.iter().copied().map(Some).collect(),
                    format!(
                        "{} logical CPUs · {:.0}% average · {:.0}% busiest core",
                        s.cpus.len(),
                        s.cpus.iter().sum::<f32>() / s.cpus.len().max(1) as f32,
                        s.cpus.iter().copied().fold(0.0, f32::max)
                    ),
                ),
                (1, Some(s)) => (vec![percent(s.ram.0, s.ram.1)], capacity(s.ram)),
                (2, Some(s)) => (vec![percent(s.swap.0, s.swap.1)], capacity(s.swap)),
                (3, Some(s)) => {
                    let selected: Vec<_> = s
                        .networks
                        .iter()
                        .filter(|(n, _)| {
                            self.settings.interface.is_empty() || **n == self.settings.interface
                        })
                        .collect();
                    if selected.is_empty() {
                        (vec![None, None], "Interface unavailable".into())
                    } else {
                        let (down, up) = selected
                            .iter()
                            .fold((0.0, 0.0), |a, (_, b)| (a.0 + b.0, a.1 + b.1));
                        let scale = f64::from(self.settings.network_mib) * 1_048_576.0;
                        (
                            vec![
                                Some((down / scale * 100.0) as f32),
                                Some((up / scale * 100.0) as f32),
                            ],
                            format!("↓ {}/s   ↑ {}/s", bytes(down), bytes(up)),
                        )
                    }
                }
                (4, Some(s)) => {
                    if let Some(disk) = s.disks.get(&self.settings.mount) {
                        let scale = f64::from(self.settings.disk_mib) * 1_048_576.0;
                        let read = disk.read_write.map(|v| (v.0 / scale * 100.0) as f32);
                        let write = disk.read_write.map(|v| (v.1 / scale * 100.0) as f32);
                        let rates = disk
                            .read_write
                            .map(|(r, w)| format!("R {}/s   W {}/s", bytes(r), bytes(w)))
                            .unwrap_or_else(|| "I/O counters unavailable".into());
                        (
                            vec![percent(disk.used, disk.total), read, write],
                            format!(
                                "{} · {}\n{rates}",
                                self.settings.mount,
                                capacity((disk.used, disk.total))
                            ),
                        )
                    } else {
                        (
                            vec![None; 3],
                            format!("{} unavailable", self.settings.mount),
                        )
                    }
                }
                (5, Some(s)) => {
                    if s.gpus.is_empty() {
                        (vec![None; 2], "GPU telemetry unavailable".into())
                    } else {
                        for (index, gpu) in s.gpus.iter().enumerate() {
                            groups.push(Group {
                                key: format!("gpu:{index}:{}", gpu.name),
                                label: gpu.name.clone(),
                                icon,
                                values: vec![gpu.busy, gpu.memory.and_then(|(u, t)| percent(u, t))],
                                detail: format!(
                                    "GPU {} · VRAM {}",
                                    gpu.busy
                                        .map(|v| format!("{v:.0}%"))
                                        .unwrap_or_else(|| "unavailable".into()),
                                    gpu.memory
                                        .map(capacity)
                                        .unwrap_or_else(|| "unavailable".into())
                                ),
                            });
                        }
                        continue;
                    }
                }
                _ => (vec![None], "Collecting first sample…".into()),
            };
            groups.push(Group {
                key: if index == 0 {
                    "cpu".into()
                } else {
                    format!("resource:{index}")
                },
                label: label.into(),
                icon,
                values,
                detail,
            });
        }
        groups
    }
    pub fn record_graph_sample(&mut self, now: std::time::Instant) {
        let groups = self.groups().into_iter().map(|g| (g.key, g.values));
        let cpus = self
            .snapshot
            .as_deref()
            .map(|s| s.cpus.as_slice())
            .unwrap_or_default();
        self.graphs.observe(cpus, groups, now, &self.settings);
    }
    pub fn panel(&self) -> Element<'_, Message> {
        let horizontal = self.core.applet.is_horizontal();
        let size = f32::from(self.core.applet.suggested_size(true).0);
        let groups = self.groups();
        let summary = groups
            .iter()
            .map(|g| format!("{}: {}", g.label, g.detail))
            .collect::<Vec<_>>()
            .join("\n");
        let units: Vec<Element<'_, Message>> = groups
            .into_iter()
            .map(|g| {
                let mut children = Vec::new();
                if self.settings.icons {
                    children.push(resource_icon(g.icon, size));
                }
                if g.key != "cpu" || self.settings.cpu_graph != CpuGraph::History {
                    children.push(graph_image(self.graphs.meter(
                        &g.key,
                        &g.values,
                        &self.settings,
                        size,
                        !horizontal,
                    )));
                }
                if g.key == "cpu" && self.settings.cpu_graph != CpuGraph::Cores {
                    children.push(graph_image(self.graphs.history(
                        &self.settings,
                        size,
                        !horizontal,
                    )));
                }
                if horizontal {
                    widget::Row::with_children(children)
                        .spacing((size / 6.0).max(2.0))
                        .align_y(Vertical::Center)
                        .into()
                } else {
                    widget::Column::with_children(children)
                        .spacing(3)
                        .align_x(Horizontal::Center)
                        .into()
                }
            })
            .collect();
        let strip: Element<'_, Message> = if units.is_empty() {
            resource_icon("processor-symbolic", size)
        } else if horizontal {
            widget::Row::with_children(units)
                .spacing((size / 3.0).max(4.0))
                .into()
        } else {
            widget::Column::with_children(units).spacing(10).into()
        };
        let (major, minor) = self.core.applet.suggested_padding(true);
        let padding = if horizontal {
            [minor as f32, major as f32]
        } else {
            [major as f32, minor as f32]
        };
        let button = widget::button::custom(strip)
            .padding(padding)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);
        let tooltip = self.core.applet.applet_tooltip(
            button,
            if summary.is_empty() {
                "Pulse — system readings".into()
            } else {
                summary
            },
            self.popup.is_some(),
            Message::Surface,
            self.core.main_window_id(),
        );
        self.core.applet.autosize_window(tooltip).into()
    }
    pub fn popup_view(&self) -> Element<'_, Message> {
        let mut content = widget::Column::new()
            .spacing(12)
            .padding(16)
            .push(widget::text::title3("Pulse"));
        for error in [self.error.as_ref(), self.settings_error.as_ref()]
            .into_iter()
            .flatten()
        {
            content = content.push(widget::text(error));
        }
        for group in self.groups() {
            content = content
                .push(widget::text::heading(group.label))
                .push(widget::text(group.detail));
        }
        let choices = [
            (CpuGraph::Cores, "Cores"),
            (CpuGraph::History, "History"),
            (CpuGraph::Both, "Both"),
        ];
        let buttons = choices.into_iter().map(|(mode, label)| {
            widget::button::text(if mode == self.settings.cpu_graph {
                format!("✓ {label}")
            } else {
                label.into()
            })
            .on_press(Message::CpuGraph(mode))
            .into()
        });
        let explanation = "History: filled area = average; dashed line = busiest core. Newest samples are on the right.";
        content = content
            .push(widget::text::heading("CPU graph"))
            .push(widget::Row::with_children(buttons).spacing(4))
            .push(widget::text::caption(explanation))
            .push(widget::settings::item(
                "Peak afterglow",
                widget::toggler(self.settings.peak_glow).on_toggle(Message::PeakGlow),
            ));
        content = content
            .push(widget::button::text("Open System Monitor").on_press(Message::OpenMonitor))
            .push(widget::text::heading("About"))
            .push(widget::text(format!(
                "Pulse {} · Resource monitor for COSMIC",
                env!("CARGO_PKG_VERSION")
            )))
            .push(widget::text("By Illya Yalovyy"))
            .push(widget::button::text("Support development").on_press(Message::Donate));
        self.core
            .applet
            .popup_container(widget::scrollable(content).height(Length::Shrink))
            .into()
    }
}

/// Embedded outline marks render independently of the installed icon theme.
fn resource_icon(name: &str, size: f32) -> Element<'static, Message> {
    static ICONS: [std::sync::OnceLock<Handle>; 6] = [const { std::sync::OnceLock::new() }; 6];
    let index = match name {
        "processor-symbolic" => 0,
        "memory-symbolic" => 1,
        "swap-symbolic" => 2,
        "network-wired-symbolic" => 3,
        "video-display-symbolic" => 4,
        _ => 5,
    };
    let handle = ICONS[index].get_or_init(|| make_icon(name));
    widget::svg(handle.clone())
        .symbolic(true)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

fn make_icon(name: &str) -> Handle {
    let shape = match name {
        "processor-symbolic" => {
            r#"<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4"/>"#
        }
        "memory-symbolic" => {
            r#"<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 9v6m5-6v6m5-6v6M7 18v3m5-3v3m5-3v3"/>"#
        }
        "swap-symbolic" => r#"<path d="M4 8h16m-4-4 4 4-4 4M20 16H4m4-4-4 4 4 4"/>"#,
        "network-wired-symbolic" => r#"<path d="M7 3v17m-4-4 4 4 4-4M17 21V4m-4 4 4-4 4 4"/>"#,
        "video-display-symbolic" => {
            r#"<rect x="3" y="3" width="18" height="14" rx="2"/><path d="M12 17v4m-5 0h10"/>"#
        }
        _ => {
            r#"<rect x="4" y="3" width="16" height="18" rx="3"/><circle cx="12" cy="10" r="3"/><path d="M8 17h8"/>"#
        }
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{shape}</g></svg>"##
    );
    Handle::from_memory(svg.into_bytes())
}

fn graph_image(image: crate::graphs::Image) -> Element<'static, Message> {
    widget::svg(image.handle)
        .width(Length::Fixed(image.width))
        .height(Length::Fixed(image.height))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{Disk, Gpu, Snapshot};
    use std::sync::Arc;

    fn app() -> App {
        let snapshot = Snapshot {
            cpus: vec![10.0, 80.0, 99.0],
            ram: (4, 8),
            swap: (0, 0),
            networks: [
                ("eth0".into(), (1_048_576.0, 524_288.0)),
                ("vpn0".into(), (1_048_576.0, 0.0)),
            ]
            .into(),
            disks: [(
                "/".into(),
                Disk {
                    used: 75,
                    total: 100,
                    read_write: Some((1_048_576.0, 0.0)),
                },
            )]
            .into(),
            gpus: vec![Gpu {
                name: "Test GPU".into(),
                busy: Some(85.0),
                memory: Some((3, 4)),
            }],
        };
        App {
            snapshot: Some(Arc::new(snapshot)),
            ..App::default()
        }
    }
    #[test]
    fn each_core_and_gpu_memory_have_independent_meters() {
        let groups = app().groups();
        assert_eq!(groups[0].values, vec![Some(10.0), Some(80.0), Some(99.0)]);
        assert_eq!(groups[1].values, vec![Some(50.0)]);
        assert_eq!(groups[2].values, vec![None]);
        assert_eq!(groups[5].values, vec![Some(85.0), Some(75.0)]);
    }
    #[test]
    fn selected_interface_and_configured_scale_control_rates() {
        let mut app = app();
        app.settings.interface = "eth0".into();
        app.settings.network_mib = 1;
        assert_eq!(app.groups()[3].values, vec![Some(100.0), Some(50.0)]);
        app.settings.interface = "unplugged".into();
        assert_eq!(app.groups()[3].values, vec![None, None]);
        app.settings.mount = "/unmounted".into();
        assert_eq!(app.groups()[4].values, vec![None; 3]);
    }
    #[test]
    fn visibility_does_not_change_other_metrics() {
        let mut app = app();
        app.settings.visible[0] = false;
        assert_eq!(app.groups()[0].label, "RAM");
        app.settings.visible = vec![false; 6];
        assert!(app.groups().is_empty());
    }
    #[test]
    fn popup_graph_controls_appear_once_even_with_multiple_or_hidden_resources() {
        use cosmic::iced::advanced::widget::{Tree, tree::Tag};
        let reference: Element<'_, Message> = widget::button::text("Example")
            .on_press(Message::Donate)
            .into();
        let button_tag = reference.as_widget().tag();
        fn count(tree: &Tree, tag: Tag) -> usize {
            usize::from(tree.tag == tag)
                + tree
                    .children
                    .iter()
                    .map(|child| count(child, tag))
                    .sum::<usize>()
        }
        let mut app = app();
        // Three display modes, System Monitor and Donate, irrespective of sensor count.
        assert_eq!(count(&Tree::new(app.popup_view()), button_tag), 5);
        app.settings.visible = vec![false; 6];
        assert_eq!(count(&Tree::new(app.popup_view()), button_tag), 5);
    }
}
