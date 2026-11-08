//! Light / Dark / System theming.
//!
//! Ported from `App/theme/theme_manager.py` and the appearance mode handling
//! of `App/views/settings_view.py`.
//!
//! The Python app calls `CTk.set_appearance_mode("Light" | "Dark" |
//! "System")` and then adjusts the text colours of the title, the database
//! status label and the navigation buttons. egui already follows the system
//! theme natively, so the port sets the theme preference and keeps the
//! explicit text colours for the elements the Python app adjusted by hand.

use egui::Color32;

/// The appearance mode of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreference {
    /// Dark text on a light background.
    Light,
    /// Light text on a dark background.
    Dark,
    /// Follow the operating system.
    #[default]
    System,
}

impl ThemePreference {
    /// Every mode, in the order the settings dropdown shows them.
    pub const ALL: [ThemePreference; 3] = [
        ThemePreference::Light,
        ThemePreference::Dark,
        ThemePreference::System,
    ];

    /// The label shown in the settings dropdown.
    pub fn label(self) -> &'static str {
        match self {
            ThemePreference::Light => "Light",
            ThemePreference::Dark => "Dark",
            ThemePreference::System => "System",
        }
    }

    /// Parse a label back into a mode.
    pub fn parse(value: &str) -> Option<ThemePreference> {
        match value {
            "Light" => Some(ThemePreference::Light),
            "Dark" => Some(ThemePreference::Dark),
            "System" => Some(ThemePreference::System),
            _ => None,
        }
    }

    /// Apply the mode to an egui context.
    pub fn apply(self, ctx: &egui::Context) {
        let visuals = match self {
            ThemePreference::Light => egui::Visuals::light(),
            ThemePreference::Dark => egui::Visuals::dark(),
            ThemePreference::System => match ctx.system_theme() {
                Some(egui::Theme::Dark) => egui::Visuals::dark(),
                _ => egui::Visuals::light(),
            },
        };
        ctx.set_visuals(visuals);
    }

    /// The colour of the main text.
    ///
    /// Mirrors `ThemeManager.apply_theme`: black in light mode, white in dark
    /// mode and the theme's own text colour in system mode.
    pub fn text_color(self, ctx: &egui::Context) -> Color32 {
        match self {
            ThemePreference::Light => Color32::BLACK,
            ThemePreference::Dark => Color32::WHITE,
            ThemePreference::System => ctx.style().visuals.text_color(),
        }
    }

    /// The colour of the subdued text — the database status label.
    pub fn sub_text_color(self, ctx: &egui::Context) -> Color32 {
        match self {
            ThemePreference::Light => Color32::from_gray(51),
            ThemePreference::Dark => Color32::from_gray(204),
            ThemePreference::System => ctx.style().visuals.text_color(),
        }
    }

    /// The background colour of a disabled control.
    pub fn disabled_bg_color(self) -> Color32 {
        match self {
            ThemePreference::Dark => Color32::from_rgb(0x1a, 0x1a, 0x1a),
            _ => Color32::from_rgb(0xf0, 0xf0, 0xf0),
        }
    }

    /// The text colour of a disabled control.
    pub fn disabled_text_color(self) -> Color32 {
        match self {
            ThemePreference::Dark => Color32::from_rgb(0xaa, 0xaa, 0xaa),
            _ => Color32::from_rgb(0x66, 0x66, 0x66),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_labels_match_the_python_appearance_modes() {
        assert_eq!(ThemePreference::Light.label(), "Light");
        assert_eq!(ThemePreference::Dark.label(), "Dark");
        assert_eq!(ThemePreference::System.label(), "System");
    }

    #[test]
    fn a_label_round_trips() {
        for mode in ThemePreference::ALL {
            assert_eq!(ThemePreference::parse(mode.label()), Some(mode));
        }
        assert_eq!(ThemePreference::parse("Sepia"), None);
    }

    #[test]
    fn system_is_the_default() {
        assert_eq!(ThemePreference::default(), ThemePreference::System);
    }

    #[test]
    fn the_disabled_colours_match_the_python_theme_manager() {
        assert_eq!(
            ThemePreference::Light.disabled_bg_color(),
            Color32::from_rgb(0xf0, 0xf0, 0xf0)
        );
        assert_eq!(
            ThemePreference::Dark.disabled_bg_color(),
            Color32::from_rgb(0x1a, 0x1a, 0x1a)
        );
        assert_eq!(
            ThemePreference::Light.disabled_text_color(),
            Color32::from_rgb(0x66, 0x66, 0x66)
        );
        assert_eq!(
            ThemePreference::Dark.disabled_text_color(),
            Color32::from_rgb(0xaa, 0xaa, 0xaa)
        );
    }
}
