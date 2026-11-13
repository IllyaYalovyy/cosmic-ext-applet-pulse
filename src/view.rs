use crate::{
    App, Message, PopupPage,
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
    resource: usize,
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
                    for mount in self.settings.selected_mounts() {
                        let (values, detail) = if let Some(disk) = s.disks.get(mount) {
                            let scale = f64::from(self.settings.disk_mib) * 1_048_576.0;
                            let read = disk.read_write.map(|v| (v.0 / scale * 100.0) as f32);
                            let write = disk.read_write.map(|v| (v.1 / scale * 100.0) as f32);
                            let rates = disk
                                .read_write
                                .map(|(r, w)| format!("R {}/s   W {}/s", bytes(r), bytes(w)))
                                .unwrap_or_else(|| "I/O counters unavailable".into());
                            (
                                vec![percent(disk.used, disk.total), read, write],
                                format!("{}\n{rates}", capacity((disk.used, disk.total))),
                            )
                        } else {
                            (vec![None; 3], "Mount unavailable".into())
                        };
                        groups.push(Group {
                            resource: index,
                            key: format!("disk:{mount}"),
                            label: format!("Disk {mount} · space / R / W"),
                            icon,
                            values,
                            detail,
                        });
                    }
                    continue;
                }
                (5, Some(s)) => {
                    if s.gpus.is_empty() {
                        (vec![None; 2], "GPU telemetry unavailable".into())
                    } else {
                        for (gpu_index, gpu) in s.gpus.iter().enumerate() {
                            groups.push(Group {
                                resource: index,
                                key: format!("gpu:{gpu_index}:{}", gpu.name),
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
                resource: index,
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
    /// Keep meter slots stable for peak tracking; the renderer compacts missing slots.
    fn panel_groups(&self) -> Vec<Group> {
        self.groups()
            .into_iter()
            .filter_map(|mut group| {
                if !self.settings.visible[group.resource] {
                    return None;
                }
                let keys: &[&str] = match group.resource {
                    3 => &["network-down", "network-up"],
                    4 => &["disk-space", "disk-read", "disk-write"],
                    5 => &["gpu-usage", "gpu-memory"],
                    _ => &[],
                };
                for (index, value) in group.values.iter_mut().enumerate() {
                    if value.is_some_and(|v| !v.is_finite())
                        || keys.get(index).is_some_and(|key| {
                            self.settings
                                .hidden_metrics
                                .iter()
                                .any(|hidden| hidden == key)
                        })
                    {
                        *value = None;
                    }
                }
                group.values.iter().any(Option::is_some).then_some(group)
            })
            .collect()
    }

    pub fn record_graph_sample(&mut self, now: std::time::Instant) {
        let groups = self.panel_groups().into_iter().map(|g| (g.key, g.values));
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
        let groups = self.panel_groups();
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
            resource_icon("view-grid-symbolic", size)
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
        let tabs = [
            (PopupPage::Readings, "Readings"),
            (PopupPage::Settings, "Settings"),
        ]
        .into_iter()
        .map(|(page, label)| {
            widget::button::text(if self.popup_page == page {
                format!("✓ {label}")
            } else {
                label.into()
            })
            .on_press(Message::PopupPage(page))
            .into()
        });
        let mut content = widget::Column::new()
            .spacing(12)
            .padding(16)
            .push(widget::text::title3("Pulse"))
            .push(widget::Row::with_children(tabs).spacing(8));
        for error in [
            self.error.as_ref(),
            self.sampling_error.as_ref(),
            self.settings_error.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            content = content.push(widget::text(error));
        }
        let page = match self.popup_page {
            PopupPage::Readings => self.readings_view(),
            PopupPage::Settings => self.preferences_view(),
        };
        content = content.push(widget::scrollable(page).height(Length::Shrink));
        self.core.applet.popup_container(content).into()
    }

    fn readings_view(&self) -> Element<'_, Message> {
        let groups = self.groups();
        let mut content = widget::Column::new().spacing(12);
        if groups.is_empty() {
            content = content.push(widget::text(
                "No resources selected. Choose what to show in Settings.",
            ));
        }
        for group in groups {
            content = content.push(
                widget::settings::section()
                    .title(group.label)
                    .add(widget::text(group.detail)),
            );
        }
        content
            .push(widget::button::text("Open System Monitor").on_press(Message::OpenMonitor))
            .into()
    }
}

/// Embedded outline marks render independently of the installed icon theme.
fn resource_icon(name: &str, size: f32) -> Element<'static, Message> {
    static ICONS: [std::sync::OnceLock<Handle>; 7] = [const { std::sync::OnceLock::new() }; 7];
    let index = match name {
        "processor-symbolic" => 0,
        "memory-symbolic" => 1,
        "swap-symbolic" => 2,
        "network-wired-symbolic" => 3,
        "video-display-symbolic" => 4,
        "view-grid-symbolic" => 6,
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
        "view-grid-symbolic" => {
            r#"<rect x="4" y="4" width="6" height="6" rx="1"/><rect x="14" y="4" width="6" height="6" rx="1"/><rect x="4" y="14" width="6" height="6" rx="1"/><rect x="14" y="14" width="6" height="6" rx="1"/>"#
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
        let groups = app().panel_groups();
        assert_eq!(groups[0].values, vec![Some(10.0), Some(80.0), Some(99.0)]);
        assert_eq!(groups[1].values, vec![Some(50.0)]);
        assert!(!groups.iter().any(|g| g.resource == 2));
        assert_eq!(groups[4].values, vec![Some(85.0), Some(75.0)]);
    }
    #[test]
    fn selected_interface_and_configured_scale_control_rates() {
        let mut app = app();
        app.settings.interface = "eth0".into();
        app.settings.network_mib = 1;
        assert_eq!(app.panel_groups()[2].values, vec![Some(100.0), Some(50.0)]);
        app.settings.interface = "unplugged".into();
        assert!(!app.panel_groups().iter().any(|g| g.resource == 3));
        app.settings.mount = "/unmounted".into();
        assert!(!app.panel_groups().iter().any(|g| g.resource == 4));
    }
    #[test]
    fn visibility_does_not_change_other_metrics() {
        let mut app = app();
        app.settings.visible[0] = false;
        assert_eq!(app.panel_groups()[0].label, "RAM");
        app.settings.visible = vec![false; 6];
        assert!(app.panel_groups().is_empty());
    }
    #[test]
    fn individual_switches_mask_only_the_selected_meter() {
        for (resource, keys) in [
            (3, vec!["network-down", "network-up"]),
            (4, vec!["disk-space", "disk-read", "disk-write"]),
            (5, vec!["gpu-usage", "gpu-memory"]),
        ] {
            for (slot, key) in keys.iter().enumerate() {
                let mut app = app();
                let original = app
                    .panel_groups()
                    .into_iter()
                    .find(|g| g.resource == resource)
                    .unwrap()
                    .values;
                app.settings.hidden_metrics = vec![(*key).into()];
                let values = app
                    .panel_groups()
                    .into_iter()
                    .find(|g| g.resource == resource)
                    .unwrap()
                    .values;
                for (i, value) in values.iter().enumerate() {
                    assert_eq!(*value, if i == slot { None } else { original[i] });
                }
                app.settings.visible[resource] = false;
                assert!(!app.panel_groups().iter().any(|g| g.resource == resource));
            }
        }
    }
    #[test]
    fn missing_sensors_hide_groups_and_partial_readings_keep_their_slots() {
        let mut app = app();
        let snapshot = Arc::make_mut(app.snapshot.as_mut().unwrap());
        snapshot.gpus[0].busy = None;
        snapshot.gpus[0].memory = None;
        snapshot.disks.get_mut("/").unwrap().read_write = None;
        assert!(!app.panel_groups().iter().any(|g| g.resource == 5));
        assert_eq!(
            app.panel_groups()
                .iter()
                .find(|g| g.resource == 4)
                .unwrap()
                .values,
            vec![Some(75.0), None, None]
        );
        let snapshot = Arc::make_mut(app.snapshot.as_mut().unwrap());
        snapshot.gpus[0].busy = Some(0.0);
        assert_eq!(
            app.panel_groups().last().unwrap().values,
            vec![Some(0.0), None]
        );
        app.settings.hidden_metrics = vec!["gpu-usage".into(), "disk-space".into()];
        assert!(!app.panel_groups().iter().any(|g| g.resource >= 4));
        assert!(app.groups().iter().any(|g| g.resource == 5));
        app.snapshot = None;
        assert!(app.panel_groups().is_empty());
    }
    #[test]
    fn mounts_have_independent_keys_and_unmounted_selections_recover() {
        let mut app = app();
        app.settings.mounts = Some(vec!["/".into(), "/home".into()]);
        assert_eq!(
            app.panel_groups()
                .iter()
                .filter(|g| g.resource == 4)
                .count(),
            1
        );
        Arc::make_mut(app.snapshot.as_mut().unwrap()).disks.insert(
            "/home".into(),
            Disk {
                used: 20,
                total: 100,
                read_write: Some((0.0, 0.0)),
            },
        );
        let groups = app.panel_groups();
        let disks: Vec<_> = groups.iter().filter(|g| g.resource == 4).collect();
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].key, "disk:/");
        assert_eq!(disks[1].key, "disk:/home");
        assert_eq!(disks[1].values, vec![Some(20.0), Some(0.0), Some(0.0)]);
        app.settings.mounts = Some(Vec::new());
        assert!(!app.panel_groups().iter().any(|g| g.resource == 4));
    }
    #[test]
    fn readings_keep_navigation_and_monitor_access_with_all_resources_hidden() {
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
        // Two page tabs and System Monitor remain reachable even with no readings.
        assert_eq!(count(&Tree::new(app.popup_view()), button_tag), 3);
        app.settings.visible = vec![false; 6];
        assert_eq!(count(&Tree::new(app.popup_view()), button_tag), 3);
    }
}
