use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

pub const APP_ID: &str = "io.github.illyayalovyy.Pulse";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CpuGraph {
    #[default]
    Cores,
    History,
    Both,
}

/// Persistent resource selection, sampling cadence and meter thresholds.
#[derive(Debug, Clone, PartialEq, Eq, CosmicConfigEntry)]
#[version = 1]
pub struct Settings {
    pub visible: Vec<bool>,
    pub icons: bool,
    pub interval: u32,
    pub warning: u32,
    pub critical: u32,
    pub network_mib: u32,
    pub disk_mib: u32,
    pub interface: String,
    pub mount: String,
    pub cpu_graph: CpuGraph,
    pub peak_glow: bool,
    pub glow_seconds: u32,
    pub history_seconds: u32,
    pub history_width: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            visible: vec![true; 6],
            icons: true,
            interval: 2,
            warning: 50,
            critical: 80,
            network_mib: 100,
            disk_mib: 500,
            interface: String::new(),
            mount: "/".into(),
            cpu_graph: CpuGraph::Cores,
            peak_glow: true,
            glow_seconds: 8,
            history_seconds: 120,
            history_width: 64,
        }
    }
}
impl Settings {
    pub fn normalized(mut self) -> Self {
        self.visible.resize(6, true);
        self.visible.truncate(6);
        self.glow_seconds = self.glow_seconds.clamp(1, 60);
        self.history_seconds = self.history_seconds.clamp(30, 120);
        self.history_width = self.history_width.clamp(32, 160);
        self.interval = self.interval.clamp(1, 30);
        self.warning = self.warning.clamp(1, 98);
        self.critical = self.critical.clamp(self.warning + 1, 100);
        self.network_mib = self.network_mib.clamp(1, 100_000);
        self.disk_mib = self.disk_mib.clamp(1, 100_000);
        self
    }
    pub fn color(&self, value: Option<f32>) -> &'static str {
        match value {
            Some(p) if p > self.critical as f32 => "#f36675",
            Some(p) if p > self.warning as f32 => "#f4ce64",
            Some(_) => "#65d69a",
            None => "#8b919c",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_config_is_repaired() {
        let s = Settings {
            warning: 200,
            critical: 0,
            interval: 0,
            visible: vec![],
            network_mib: 0,
            history_width: u32::MAX,
            history_seconds: u32::MAX,
            glow_seconds: 0,
            ..Settings::default()
        }
        .normalized();
        assert_eq!(
            (s.warning, s.critical, s.interval, s.network_mib),
            (98, 99, 1, 1)
        );
        assert_eq!(s.visible.len(), 6);
        assert_eq!(
            (s.history_width, s.history_seconds, s.glow_seconds),
            (160, 120, 1)
        );
    }
    #[test]
    fn colors_include_threshold_boundaries_and_unknown() {
        let s = Settings::default();
        for value in [0.0, 49.9, 50.0] {
            assert_eq!(s.color(Some(value)), "#65d69a");
        }
        for value in [50.1, 79.9, 80.0] {
            assert_eq!(s.color(Some(value)), "#f4ce64");
        }
        for value in [80.1, 100.0] {
            assert_eq!(s.color(Some(value)), "#f36675");
        }
        assert_eq!(s.color(None), "#8b919c");
    }
    #[test]
    fn preferences_survive_a_config_store_round_trip() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("pulse-config-{}-{unique}", std::process::id()));
        let store =
            cosmic_config::Config::with_custom_path(APP_ID, Settings::VERSION, directory.clone())
                .unwrap();
        let expected = Settings {
            interval: 5,
            warning: 60,
            critical: 85,
            interface: "eth0".into(),
            mount: "/home".into(),
            cpu_graph: CpuGraph::Both,
            peak_glow: false,
            glow_seconds: 12,
            history_seconds: 60,
            history_width: 96,
            ..Settings::default()
        };
        expected.write_entry(&store).unwrap();
        let reloaded = Settings::get_entry(&store).unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!(reloaded, expected);
    }
    #[test]
    fn old_settings_keep_their_values_and_default_new_graph_fields() {
        use cosmic::cosmic_config::ConfigSet;
        let directory =
            std::env::temp_dir().join(format!("pulse-migration-{}", std::process::id()));
        let store =
            cosmic_config::Config::with_custom_path(APP_ID, Settings::VERSION, directory.clone())
                .unwrap();
        store.set("interval", 7_u32).unwrap();
        store.set("warning", 62_u32).unwrap();
        let settings = Settings::get_entry(&store)
            .unwrap_or_else(|(_, s)| s)
            .normalized();
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!((settings.interval, settings.warning), (7, 62));
        assert_eq!(settings.cpu_graph, CpuGraph::Cores);
        assert!(settings.peak_glow);
        assert_eq!(settings.history_seconds, 120);
    }
}
