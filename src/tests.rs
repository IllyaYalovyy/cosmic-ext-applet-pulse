use super::*;
use cosmic::Application;

#[test]
fn sampling_recovers_without_erasing_unrelated_errors() {
    let mut app = App {
        error: Some("Monitor is not installed".into()),
        settings_error: Some("Settings are read-only".into()),
        ..App::default()
    };
    let snapshot = Arc::new(metrics::Snapshot {
        cpus: vec![100.0, 0.0],
        ..metrics::Snapshot::default()
    });
    let _ = app.update(Message::Sample(snapshot.clone()));
    let _ = app.update(Message::SamplingError);
    assert!(app.snapshot.is_none());
    assert!(app.sampling_error.is_some());
    let _ = app.update(Message::Sample(snapshot));
    assert!(app.snapshot.is_some());
    assert!(app.sampling_error.is_none());
    assert_eq!(app.error.as_deref(), Some("Monitor is not installed"));
    assert_eq!(
        app.settings_error.as_deref(),
        Some("Settings are read-only")
    );
}

#[test]
fn watched_settings_are_normalized_before_use() {
    let mut app = App::default();
    let _ = app.update(Message::Config(Settings {
        visible: vec![],
        interval: 0,
        network_mib: 0,
        warning: u32::MAX,
        critical: 0,
        ..Settings::default()
    }));
    assert_eq!(app.settings.visible, vec![true; 6]);
    assert_eq!(app.settings.interval, 1);
    assert_eq!(app.settings.network_mib, 1);
    assert!(app.settings.critical > app.settings.warning);
}

#[test]
fn popup_preferences_apply_even_when_storage_is_unavailable() {
    let mut app = App::default();
    let _ = app.update(Message::CpuGraph(CpuGraph::Both));
    let _ = app.update(Message::PeakGlow(false));
    assert_eq!(app.settings.cpu_graph, CpuGraph::Both);
    assert!(!app.settings.peak_glow);
    assert!(app.settings_error.is_some());
}

#[test]
fn unrelated_window_close_does_not_close_popup() {
    let popup = Id::unique();
    let mut app = App {
        popup: Some(popup),
        ..App::default()
    };
    let _ = app.update(Message::Closed(Id::unique()));
    assert_eq!(app.popup, Some(popup));
    let _ = app.update(Message::Closed(popup));
    assert_eq!(app.popup, None);
}

#[test]
fn panel_preferences_and_multiple_mounts_apply_without_storage() {
    let mut app = App::default();
    let _ = app.update(Message::ResourceVisibility(1, false));
    let _ = app.update(Message::MetricVisibility("gpu-memory".into(), false));
    let _ = app.update(Message::MetricVisibility("gpu-memory".into(), false));
    let _ = app.update(Message::MountVisibility("/home".into(), true));
    assert!(!app.settings.visible[1]);
    assert_eq!(app.settings.hidden_metrics, ["gpu-memory"]);
    assert_eq!(app.settings.selected_mounts(), &["/", "/home"]);
    let _ = app.update(Message::MetricVisibility("gpu-memory".into(), true));
    let _ = app.update(Message::MountVisibility("/".into(), false));
    let _ = app.update(Message::MountVisibility("/home".into(), false));
    assert!(app.settings.hidden_metrics.is_empty());
    assert!(app.settings.selected_mounts().is_empty());
    assert!(app.settings_error.is_some());
}

#[test]
fn preferences_navigation_does_not_change_or_save_configuration() {
    let mut app = App::default();
    let original = app.settings.clone();
    let _ = app.update(Message::PopupPage(PopupPage::Settings));
    let _ = app.update(Message::ResourceOptions(4));
    assert_eq!(app.expanded_resource, Some(4));
    let _ = app.update(Message::ResourceOptions(3));
    assert_eq!(app.expanded_resource, Some(3));
    let _ = app.update(Message::ResourceOptions(3));
    assert_eq!(app.expanded_resource, None);
    let _ = app.update(Message::PopupPage(PopupPage::Readings));
    assert_eq!(app.settings, original);
    assert!(app.settings_error.is_none());
}

#[test]
fn common_display_preferences_apply_and_normalize() {
    let mut app = App::default();
    let _ = app.update(Message::Icons(false));
    let _ = app.update(Message::Interval(5));
    let _ = app.update(Message::GlowSeconds(15));
    assert!(!app.settings.icons);
    assert_eq!((app.settings.interval, app.settings.glow_seconds), (5, 15));
    let _ = app.update(Message::Interval(0));
    let _ = app.update(Message::GlowSeconds(u32::MAX));
    assert_eq!((app.settings.interval, app.settings.glow_seconds), (1, 60));
    assert!(app.settings_error.is_some());
}
