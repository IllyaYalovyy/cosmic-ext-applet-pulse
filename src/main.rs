mod config;
mod graphs;
mod metrics;
mod view;

use config::{APP_ID, CpuGraph, Settings};
use cosmic::{
    cosmic_config::{self, CosmicConfigEntry},
    iced::{
        Limits, Subscription,
        futures::{self, SinkExt},
        platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
        window::Id,
    },
    prelude::*,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Panel state and the latest asynchronously collected readings.
#[derive(Default)]
struct App {
    core: cosmic::Core,
    graphs: graphs::Graphs,
    store: Option<cosmic_config::Config>,
    popup: Option<Id>,
    settings: Settings,
    snapshot: Option<Arc<metrics::Snapshot>>,
    error: Option<String>,
    sampling_error: Option<String>,
    settings_error: Option<String>,
}
/// Events from the panel, configuration watcher and sampling worker.
#[derive(Clone, Debug)]
enum Message {
    Surface(cosmic::surface::Action),
    TogglePopup,
    Closed(Id),
    Sample(Arc<metrics::Snapshot>),
    SamplingError,
    Config(Settings),
    OpenMonitor,
    Donate,
    CpuGraph(CpuGraph),
    PeakGlow(bool),
    ResourceVisibility(usize, bool),
    MetricVisibility(String, bool),
    MountVisibility(String, bool),
}
impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;
    fn core(&self) -> &cosmic::Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }
    fn init(core: cosmic::Core, _: ()) -> (Self, Task<cosmic::Action<Message>>) {
        let (store, settings_error) = match cosmic_config::Config::new(APP_ID, Settings::VERSION) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(format!("Settings cannot be saved: {e}"))),
        };
        let settings = store
            .as_ref()
            .map(|c| Settings::get_entry(c).unwrap_or_else(|(_, s)| s))
            .unwrap_or_default()
            .normalized();
        (
            Self {
                core,
                settings,
                store,
                settings_error,
                ..Self::default()
            },
            Task::none(),
        )
    }
    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::Closed(id))
    }
    fn view(&self) -> Element<'_, Message> {
        self.panel()
    }
    fn view_window(&self, _: Id) -> Element<'_, Message> {
        self.popup_view()
    }
    fn subscription(&self) -> Subscription<Message> {
        let interval = self.settings.interval;
        let sampling = Subscription::run_with(interval, |interval| {
            let interval = *interval;
            cosmic::iced::stream::channel(
                1,
                move |mut output: futures::channel::mpsc::Sender<Message>| async move {
                    let Ok(mut collector) =
                        tokio::task::spawn_blocking(metrics::Collector::new).await
                    else {
                        let _ = output.send(Message::SamplingError).await;
                        return;
                    };
                    loop {
                        tokio::time::sleep(Duration::from_secs(u64::from(interval))).await;
                        let Ok((next, snapshot)) = tokio::task::spawn_blocking(move || {
                            let snapshot = collector.sample();
                            (collector, snapshot)
                        })
                        .await
                        else {
                            let _ = output.send(Message::SamplingError).await;
                            return;
                        };
                        collector = next;
                        if output
                            .send(Message::Sample(Arc::new(snapshot)))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                },
            )
        });
        Subscription::batch([
            sampling,
            self.core
                .watch_config::<Settings>(APP_ID)
                .map(|u| Message::Config(u.config)),
        ])
    }
    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }
            Message::TogglePopup => {
                return if let Some(id) = self.popup.take() {
                    destroy_popup(id)
                } else {
                    let Some(parent) = self.core.main_window_id() else {
                        return Task::none();
                    };
                    let id = Id::unique();
                    self.popup = Some(id);
                    let mut settings = self
                        .core
                        .applet
                        .get_popup_settings(parent, id, None, None, None);
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(420.0)
                        .max_width(460.0)
                        .max_height(720.0);
                    get_popup(settings)
                };
            }
            Message::Closed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                return Task::none();
            }
            Message::SamplingError => {
                self.snapshot = None;
                self.graphs.clear();
                self.sampling_error = Some("Sampling stopped unexpectedly. Restart Pulse.".into());
                return Task::none();
            }
            Message::Sample(snapshot) => {
                self.sampling_error = None;
                self.snapshot = Some(snapshot);
                self.record_graph_sample(Instant::now());
                return Task::none();
            }
            Message::Config(settings) => {
                self.apply_settings(settings);
                return Task::none();
            }
            Message::CpuGraph(mode) => {
                let mut settings = self.settings.clone();
                settings.cpu_graph = mode;
                self.apply_settings(settings);
                self.save_settings();
            }
            Message::PeakGlow(enabled) => {
                let mut settings = self.settings.clone();
                settings.peak_glow = enabled;
                self.apply_settings(settings);
                self.save_settings();
            }
            Message::ResourceVisibility(index, enabled) => {
                let mut settings = self.settings.clone();
                if let Some(visible) = settings.visible.get_mut(index) {
                    *visible = enabled;
                }
                self.apply_settings(settings);
                self.save_settings();
            }
            Message::MetricVisibility(key, enabled) => {
                let mut settings = self.settings.clone();
                settings.hidden_metrics.retain(|metric| metric != &key);
                if !enabled {
                    settings.hidden_metrics.push(key);
                }
                self.apply_settings(settings);
                self.save_settings();
            }
            Message::MountVisibility(mount, enabled) => {
                let mut settings = self.settings.clone();
                let mut mounts = settings.selected_mounts().to_vec();
                mounts.retain(|selected| selected != &mount);
                if enabled {
                    mounts.push(mount);
                }
                settings.mounts = Some(mounts);
                self.apply_settings(settings);
                self.save_settings();
            }
            Message::OpenMonitor => {
                let result = ["cosmic-monitor", "gnome-system-monitor"]
                    .into_iter()
                    .find_map(|program| std::process::Command::new(program).spawn().ok());
                if result.is_none() {
                    self.error =
                        Some("Install COSMIC System Monitor or GNOME System Monitor.".into());
                }
            }
            Message::Donate => {
                if let Err(error) = std::process::Command::new("xdg-open")
                    .arg("https://github.com/sponsors/IllyaYalovyy")
                    .spawn()
                {
                    self.error = Some(format!("Could not open donation page: {error}"));
                }
            }
        }
        Task::none()
    }
    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
impl App {
    fn apply_settings(&mut self, settings: Settings) {
        let settings = settings.normalized();
        if self.settings.interface != settings.interface
            || self.settings.mount != settings.mount
            || self.settings.mounts != settings.mounts
            || self.settings.visible != settings.visible
            || self.settings.hidden_metrics != settings.hidden_metrics
            || self.settings.network_mib != settings.network_mib
            || self.settings.disk_mib != settings.disk_mib
            || self.settings.peak_glow != settings.peak_glow
        {
            self.graphs.clear_peaks();
        }
        self.settings = settings;
    }
    fn save_settings(&mut self) {
        self.settings_error = match &self.store {
            Some(store) => self
                .settings
                .write_entry(store)
                .err()
                .map(|e| format!("Could not save settings: {e}")),
            None => {
                Some("Settings storage is unavailable; changes apply for this session only.".into())
            }
        };
    }
}
fn main() -> cosmic::iced::Result {
    if std::env::args().any(|a| a == "--sample") {
        let mut collector = metrics::Collector::new();
        std::thread::sleep(Duration::from_secs(1));
        println!("{:#?}", collector.sample());
        return Ok(());
    }
    cosmic::applet::run::<App>(())
}

#[cfg(test)]
mod tests;
