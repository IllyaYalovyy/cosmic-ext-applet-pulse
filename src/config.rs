use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

pub const APP_ID: &str = "io.github.illyayalovyy.Pulse";

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
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            visible: vec![true; 6],
            icons: true,
            interval: 2,
            warning: 70,
            critical: 90,
            network_mib: 100,
            disk_mib: 500,
            interface: String::new(),
            mount: "/".into(),
        }
    }
}
impl Settings {
    pub fn normalized(mut self) -> Self {
        self.visible.resize(6, true);
        self.visible.truncate(6);
        self.interval = self.interval.clamp(1, 30);
        self.warning = self.warning.clamp(1, 98);
        self.critical = self.critical.clamp(self.warning + 1, 100);
        self.network_mib = self.network_mib.clamp(1, 100_000);
        self.disk_mib = self.disk_mib.clamp(1, 100_000);
        self
    }
    pub fn color(&self, value: Option<f32>) -> &'static str {
        match value {
            Some(p) if p >= self.critical as f32 => "#f36675",
            Some(p) if p >= self.warning as f32 => "#f4ce64",
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
            ..Settings::default()
        }
        .normalized();
        assert_eq!(
            (s.warning, s.critical, s.interval, s.network_mib),
            (98, 99, 1, 1)
        );
        assert_eq!(s.visible.len(), 6);
    }
    #[test]
    fn colors_include_threshold_boundaries_and_unknown() {
        let s = Settings::default();
        assert_eq!(s.color(Some(69.9)), "#65d69a");
        assert_eq!(s.color(Some(70.0)), "#f4ce64");
        assert_eq!(s.color(Some(90.0)), "#f36675");
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
            ..Settings::default()
        };
        expected.write_entry(&store).unwrap();
        let reloaded = Settings::get_entry(&store).unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!(reloaded, expected);
    }
}
