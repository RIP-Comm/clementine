//! Persistent UI settings, stored next to the binary so preferences survive
//! between runs.

use std::path::PathBuf;

use egui::Key;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    /// Name of the hold-to-fast-forward key (see [`egui::Key::name`]).
    pub fast_forward_key: String,
    /// Emulation speed multiplier.
    pub speed: f32,
    /// Master audio volume, 0.0 to 1.0.
    pub volume: f32,
    /// Whether audio is muted.
    pub muted: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fast_forward_key: Key::Space.name().to_string(),
            speed: 1.0,
            volume: 1.0,
            muted: false,
        }
    }
}

impl Settings {
    /// Where the settings file lives: next to the executable, falling back to
    /// the working directory if the executable path is unavailable.
    #[must_use]
    pub fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default()
            .join("clementine_settings.bin")
    }

    /// Load settings from disk, returning defaults if the file is missing or
    /// cannot be read.
    #[must_use]
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_or_else(|_| Self::default(), |(settings, _)| settings)
    }

    /// Write settings to disk atomically. Errors are ignored, since failing to
    /// persist preferences must never disrupt the emulator.
    pub fn save(&self, path: &std::path::Path) {
        let Ok(bytes) = bincode::serde::encode_to_vec(self, bincode::config::standard()) else {
            return;
        };
        let tmp = path.with_extension("bin.tmp");
        let _ = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    }

    /// Resolve the stored fast-forward key name back to a [`Key`], defaulting to
    /// Space if the name is not recognized.
    #[must_use]
    pub fn fast_forward_key(&self) -> Key {
        Key::ALL
            .iter()
            .copied()
            .find(|key| key.name() == self.fast_forward_key)
            .unwrap_or(Key::Space)
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use egui::Key;

    #[test]
    fn fast_forward_key_round_trips_through_the_name() {
        let settings = Settings {
            fast_forward_key: Key::Tab.name().to_string(),
            ..Default::default()
        };
        assert_eq!(settings.fast_forward_key(), Key::Tab);

        // An unknown name falls back to Space.
        let settings = Settings {
            fast_forward_key: "NotAKey".to_string(),
            ..Default::default()
        };
        assert_eq!(settings.fast_forward_key(), Key::Space);
    }

    #[test]
    fn encode_decode_round_trips() {
        let settings = Settings {
            fast_forward_key: Key::Tab.name().to_string(),
            speed: 4.0,
            volume: 0.5,
            muted: true,
        };
        let bytes = bincode::serde::encode_to_vec(&settings, bincode::config::standard()).unwrap();
        let (decoded, _): (Settings, _) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(decoded.fast_forward_key(), Key::Tab);
        assert!((decoded.speed - 4.0).abs() < f32::EPSILON);
        assert!((decoded.volume - 0.5).abs() < f32::EPSILON);
        assert!(decoded.muted);
    }
}
