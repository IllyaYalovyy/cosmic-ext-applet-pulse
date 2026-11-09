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
