//! Popup preferences: shared display controls and resource-specific disclosure.
use crate::{
    App, Message,
    config::{CpuGraph, METERS, RESOURCES},
};
use cosmic::{iced::Length, prelude::*, widget};

impl App {
    pub fn preferences_view(&self) -> Element<'_, Message> {
        let display = widget::settings::section()
            .title("Display & updates")
            .add(widget::settings::item(
                "Resource icons",
                widget::toggler(self.settings.icons).on_toggle(Message::Icons),
            ))
            .add(widget::settings::item(
                "Refresh every",
                seconds_choice(
                    self.settings.interval,
                    &[1, 2, 5, 10, 30],
                    Message::Interval,
                ),
            ))
            .add(widget::text::caption(
                "Slower updates use fewer resources. Changes save automatically.",
            ))
            .add(widget::settings::item(
                "Peak afterglow",
                widget::toggler(self.settings.peak_glow).on_toggle(Message::PeakGlow),
            ))
            .add_maybe(self.settings.peak_glow.then(|| {
                widget::settings::item(
                    "Fade duration",
                    seconds_choice(
                        self.settings.glow_seconds,
                        &[4, 8, 15, 30, 60],
                        Message::GlowSeconds,
                    ),
                )
            }))
            .add_maybe(self.settings.peak_glow.then(|| {
                widget::text::caption(
                    "Peaks fade as readings update, over at least two refresh intervals.",
                )
            }));
        let mut content = widget::Column::new().spacing(16).push(display)
            .push(widget::text::heading("Resources"))
            .push(widget::text::caption("Choose what appears in the panel or dock. Unavailable meters hide automatically; zero readings stay visible."));
        for (index, label) in RESOURCES.into_iter().enumerate() {
            let expanded = self.expanded_resource == Some(index);
            let has_options = matches!(index, 0 | 3 | 4 | 5);
            let name: Element<'_, Message> = if has_options {
                widget::button::text(format!("{} {label}", if expanded { "▾" } else { "▸" }))
                    .on_press(Message::ResourceOptions(index))
                    .width(Length::Fill)
                    .into()
            } else {
                widget::text(label).width(Length::Fill).into()
            };
            let header = widget::Row::new().spacing(8).push(name).push(
                widget::toggler(self.settings.visible[index])
                    .on_toggle(move |enabled| Message::ResourceVisibility(index, enabled)),
            );
            let mut section = widget::settings::section().add(header);
            if expanded && has_options {
                if !self.settings.visible[index] {
                    section = section.add(widget::text::caption("Hidden from the panel. Your options are kept for when you enable it again."));
                }
                if index == 0 {
                    let choices = [
                        (CpuGraph::Cores, "Per core"),
                        (CpuGraph::History, "History"),
                        (CpuGraph::Both, "Both"),
                    ];
                    let buttons = choices.into_iter().map(|(mode, label)| {
                        widget::button::text(if self.settings.cpu_graph == mode {
                            format!("✓ {label}")
                        } else {
                            label.into()
                        })
                        .on_press(Message::CpuGraph(mode))
                        .into()
                    });
                    section = section.add(widget::text::heading("Graph style"))
                        .add(widget::Row::with_children(buttons).spacing(4))
                        .add(widget::text::caption("History shows average load as a filled area and the busiest core as a dashed line."));
                }
                for (key, label) in METERS.into_iter().filter(|(key, _)| match index {
                    3 => key.starts_with("network-"),
                    4 => key.starts_with("disk-"),
                    5 => key.starts_with("gpu-"),
                    _ => false,
                }) {
                    section = section.add(widget::settings::item(
                        label,
                        widget::toggler(
                            !self
                                .settings
                                .hidden_metrics
                                .iter()
                                .any(|hidden| hidden == key),
                        )
                        .on_toggle(move |enabled| Message::MetricVisibility(key.into(), enabled)),
                    ));
                }
                if index == 4 {
                    section = section.add(widget::text::heading("Mounts to monitor"));
                    for (mount, available, selected) in self.mount_options() {
                        let label = if available {
                            mount.clone()
                        } else {
                            format!("{mount} · unavailable")
                        };
                        section = section.add(widget::settings::item(
                            label,
                            widget::toggler(selected).on_toggle(move |enabled| {
                                Message::MountVisibility(mount.clone(), enabled)
                            }),
                        ));
                    }
                    if self.settings.selected_mounts().is_empty() {
                        section = section
                            .add(widget::text::caption("Select a mount to show disk meters."));
                    }
                }
            }
            content = content.push(section);
        }
        content
            .push(
                widget::settings::section()
                    .title("About Pulse")
                    .add(widget::text(format!(
                        "Version {} · By Illya Yalovyy",
                        env!("CARGO_PKG_VERSION")
                    ))),
            )
            .push(widget::button::text("Support development").on_press(Message::Donate))
            .into()
    }

    fn mount_options(&self) -> Vec<(String, bool, bool)> {
        let mut mounts = std::collections::BTreeSet::new();
        mounts.extend(self.settings.selected_mounts().iter().cloned());
        if let Some(snapshot) = &self.snapshot {
            mounts.extend(snapshot.disks.keys().cloned());
        }
        mounts
            .into_iter()
            .map(|mount| {
                let available = self
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.disks.contains_key(&mount));
                let selected = self.settings.selected_mounts().contains(&mount);
                (mount, available, selected)
            })
            .collect()
    }
}

/// Keep custom cosmic-config values visible rather than silently selecting a preset.
fn seconds_choice(
    current: u32,
    presets: &[u32],
    message: fn(u32) -> Message,
) -> Element<'static, Message> {
    let mut values = presets.to_vec();
    values.push(current);
    values.sort_unstable();
    values.dedup();
    let selected = values.iter().position(|value| *value == current);
    let labels: Vec<_> = values.iter().map(|value| format!("{value} s")).collect();
    widget::dropdown(labels, selected, move |index| message(values[index])).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::advanced::widget::{Tree, tree::Tag};

    fn count(tree: &Tree, tag: Tag) -> usize {
        usize::from(tree.tag == tag)
            + tree
                .children
                .iter()
                .map(|child| count(child, tag))
                .sum::<usize>()
    }

    #[test]
    fn only_expanded_resource_exposes_its_meter_switches() {
        let reference: Element<'_, Message> =
            widget::toggler(true).on_toggle(Message::PeakGlow).into();
        let tag = reference.as_widget().tag();
        let mut app = App::default();
        // Icons, afterglow and six resource switches; detailed meters start collapsed.
        assert_eq!(count(&Tree::new(app.preferences_view()), tag), 8);
        app.expanded_resource = Some(3);
        assert_eq!(count(&Tree::new(app.preferences_view()), tag), 10);
        app.expanded_resource = Some(4);
        assert_eq!(count(&Tree::new(app.preferences_view()), tag), 12);
        app.expanded_resource = Some(5);
        assert_eq!(count(&Tree::new(app.preferences_view()), tag), 10);
        app.settings.visible[5] = false;
        assert_eq!(count(&Tree::new(app.preferences_view()), tag), 10);
    }

    #[test]
    fn mount_picker_preserves_unavailable_selections_and_deduplicates_available_mounts() {
        let app = App {
            settings: crate::config::Settings {
                mounts: Some(vec!["/offline".into(), "/".into()]),
                ..Default::default()
            },
            snapshot: Some(std::sync::Arc::new(crate::metrics::Snapshot {
                disks: [
                    ("/".into(), Default::default()),
                    ("/home".into(), Default::default()),
                ]
                .into(),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            app.mount_options(),
            vec![
                ("/".into(), true, true),
                ("/home".into(), true, false),
                ("/offline".into(), false, true)
            ]
        );
    }
}
