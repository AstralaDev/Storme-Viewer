//! Storme's built-in themes (Preferences › Appearance › Skin): a picker of
//! ready-made looks that switch at once, without a restart. Each is a
//! `theme::Theme` (the colours, corner radii and glass of the skin), so
//! everything built on the palette follows: windows, bars, buttons, chat,
//! name plates. The accent, background and text colours of
//! `crate::interface_colors`, high contrast, colour blindness adaptation,
//! the UI scale and Reduce motion apply on top of whichever theme is chosen.
//!
//! `Storme` (0) is the default and the fallback: the skin's own
//! `theme.json` (Aurora's palette without one), so a hand-edited skin
//! still works. Any unknown number (a newer build's theme) reads as it.
//! Numbers are stored in the settings and never reused.
//!
//! Two process-wide controls shape every theme, like `interface_colors`:
//! - the glass: a share of the panels' own opacity (`set_glass`);
//! - the animation intensity: a share of egui's transition time
//!   (`theme::apply_motion`).
//!
//! Contrast is measured in `tests` for every theme: text 4.5:1 on its
//! panel (WCAG AA), the dim text, the accent and the state colours 3:1.

use crate::theme::Theme;
use std::sync::atomic::{AtomicU8, Ordering};

/// A built-in theme. The numbers are the stored settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Storme = 0,
    WhiteMatte = 1,
    HelloKitty = 2,
    Kuromi = 3,
    Sakura = 4,
    LoFi = 5,
    Gothic = 6,
}

/// In the order of the picker.
pub const ALL: [ThemeId; 7] = [
    ThemeId::Storme,
    ThemeId::WhiteMatte,
    ThemeId::HelloKitty,
    ThemeId::Kuromi,
    ThemeId::Sakura,
    ThemeId::LoFi,
    ThemeId::Gothic,
];

impl ThemeId {
    /// The theme of a stored number; the default for one this build does
    /// not know.
    pub fn from_u8(n: u8) -> ThemeId {
        ALL.iter().copied().find(|t| *t as u8 == n).unwrap_or(ThemeId::Storme)
    }

    pub fn name(self) -> &'static str {
        use aurora_i18n::tr;
        match self {
            ThemeId::Storme => tr!("ui-theme-storme"),
            ThemeId::WhiteMatte => tr!("ui-theme-white-matte"),
            ThemeId::HelloKitty => tr!("ui-theme-hello-kitty"),
            ThemeId::Kuromi => tr!("ui-theme-kuromi"),
            ThemeId::Sakura => tr!("ui-theme-sakura"),
            ThemeId::LoFi => tr!("ui-theme-lofi"),
            ThemeId::Gothic => tr!("ui-theme-gothic"),
        }
    }

    /// The colours of a built-in theme; `None` for `Storme`, which is the
    /// skin's own.
    pub fn theme(self) -> Option<Theme> {
        let t = match self {
            ThemeId::Storme => return None,
            ThemeId::WhiteMatte => theme(&Colors {
                accent: ["#5B5BD6", "#4A4AB8", "#7373E0", "#A9A9EE", "#6C6CD8"],
                surfaces: ["#F3F3F1", "#E6E6E3", "#FFFFFF", "#DCDCD8", "#FAFAF8"],
                text: ["#23252B", "#565A66", "#6B6F7B"],
                link: "#0B6E6E",
                states: ["#7A5000", "#B8326F", "#B3261E", "#1E7A3C", "#A04A00", "#C25A52"],
                on_accent: "#FFFFFF",
                alpha: 240,
                radii: (6, 4),
            }),
            ThemeId::HelloKitty => theme(&Colors {
                accent: ["#D93A73", "#C2305F", "#EC6C97", "#F6A8C3", "#D8527F"],
                surfaces: ["#FFF1F5", "#FFE0EA", "#FFFFFF", "#FFD0DF", "#FFF8FA"],
                text: ["#4A1D2E", "#7A3F55", "#96566E"],
                link: "#0E6F7E",
                states: ["#7A5000", "#C2305F", "#B3261E", "#1E7A3C", "#A04A00", "#C25A52"],
                on_accent: "#FFFFFF",
                alpha: 235,
                radii: (10, 8),
            }),
            ThemeId::Kuromi => theme(&Colors {
                accent: ["#B44CF0", "#7B2FBE", "#D08CFF", "#E6C2FF", "#A15BE0"],
                surfaces: ["#1E1327", "#150C1D", "#120A19", "#34204A", "#0B0610"],
                text: ["#F3E8FF", "#C4A9E3", "#9C80BF"],
                link: "#FF7AC6",
                states: ["#FFD166", "#FF6FB5", "#FF7A7A", "#6EE7A0", "#FFA94D", "#B0525F"],
                on_accent: "#FFFFFF",
                alpha: 232,
                radii: (8, 6),
            }),
            ThemeId::Sakura => theme(&Colors {
                accent: ["#C94A74", "#A93A5D", "#E07796", "#F2B3C6", "#D4607F"],
                surfaces: ["#FDF4F6", "#F9E6EC", "#FFFFFF", "#F3D3DD", "#FFFAFB"],
                text: ["#3B2A31", "#6A4C57", "#82616C"],
                link: "#2F6F4D",
                states: ["#7A5000", "#B8326F", "#B3261E", "#1E7A3C", "#A04A00", "#C25A52"],
                on_accent: "#FFFFFF",
                alpha: 238,
                radii: (12, 10),
            }),
            ThemeId::LoFi => theme(&Colors {
                accent: ["#D9A066", "#B7794A", "#E8BE8C", "#F2D8B8", "#CC9460"],
                surfaces: ["#2A2522", "#1F1B19", "#1A1614", "#3D3631", "#151210"],
                text: ["#EFE6DA", "#BFB2A0", "#968A79"],
                link: "#7FBFA8",
                states: ["#F2C879", "#E58F9A", "#F08A80", "#8FD19E", "#F0A868", "#A85F58"],
                on_accent: "#1A1614",
                alpha: 235,
                radii: (6, 4),
            }),
            ThemeId::Gothic => theme(&Colors {
                accent: ["#C93550", "#8E1F34", "#E0566F", "#F2A0B0", "#D14560"],
                surfaces: ["#17141A", "#0F0D12", "#0C0A0F", "#2B2530", "#08070A"],
                text: ["#E9E3EC", "#B5AABD", "#8D8196"],
                link: "#9AA8D6",
                states: ["#E6C06A", "#E8789A", "#FF7A7A", "#7ED6A0", "#F2A25C", "#A85258"],
                on_accent: "#FFFFFF",
                alpha: 235,
                radii: (3, 2),
            }),
        };
        Some(t)
    }
}

/// One theme's colours, as written in `theme.json` terms.
struct Colors {
    /// violet, indigo, violet_light, violet_pale, indigo_light
    accent: [&'static str; 5],
    /// panel, bar, field, raised, navy
    surfaces: [&'static str; 5],
    /// ink, muted, muted_dim
    text: [&'static str; 3],
    /// the links (`teal`)
    link: &'static str,
    /// amber, rose, danger, success, warn, danger_dim
    states: [&'static str; 6],
    on_accent: &'static str,
    alpha: u8,
    /// button radius, window radius
    radii: (u8, u8),
}

fn theme(c: &Colors) -> Theme {
    let s = |v: &str| v.to_owned();
    Theme {
        violet: s(c.accent[0]),
        indigo: s(c.accent[1]),
        violet_light: s(c.accent[2]),
        violet_pale: s(c.accent[3]),
        indigo_light: s(c.accent[4]),
        panel: s(c.surfaces[0]),
        bar: s(c.surfaces[1]),
        field: s(c.surfaces[2]),
        raised: s(c.surfaces[3]),
        navy: s(c.surfaces[4]),
        ink: s(c.text[0]),
        muted: s(c.text[1]),
        muted_dim: s(c.text[2]),
        teal: s(c.link),
        amber: s(c.states[0]),
        rose: s(c.states[1]),
        danger: s(c.states[2]),
        success: s(c.states[3]),
        warn: s(c.states[4]),
        danger_dim: s(c.states[5]),
        on_accent: s(c.on_accent),
        panel_alpha: c.alpha,
        button_radius: c.radii.0,
        window_radius: c.radii.1,
        font_scale: 1.0,
    }
}

/// The glass, in percent of the panels' own opacity (100: as the theme
/// gives it).
static GLASS: AtomicU8 = AtomicU8::new(100);

/// The lowest glass: below it a window no longer reads against the world.
pub const GLASS_MIN: f32 = 0.35;

/// A glass share in percent (35..=100); garbage reads as no glass.
fn glass_percent(share: f32) -> u8 {
    let share = if share.is_finite() { share.clamp(GLASS_MIN, 1.0) } else { 1.0 };
    (share * 100.0).round() as u8
}

/// A panel opacity with `percent` of glass applied.
fn scaled(alpha: u8, percent: u8) -> u8 {
    (alpha as f32 * percent as f32 / 100.0).round() as u8
}

/// Set the glass (0.35..=1). True when it changed.
pub fn set_glass(share: f32) -> bool {
    let percent = glass_percent(share);
    GLASS.swap(percent, Ordering::Relaxed) != percent
}

/// A panel opacity (0..=255) with the glass applied.
pub fn glass_alpha(alpha: u8) -> u8 {
    scaled(alpha, GLASS.load(Ordering::Relaxed))
}

/// egui's transition time with the animation intensity applied
/// (0: none, 1: egui's own).
pub fn animation_time(intensity: f32) -> f32 {
    let intensity = if intensity.is_finite() { intensity.clamp(0.0, 1.0) } else { 1.0 };
    egui::Style::default().animation_time * intensity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface_colors::contrast;
    use crate::theme::hex;

    fn built_in() -> impl Iterator<Item = (ThemeId, Theme)> {
        ALL.into_iter().filter_map(|id| id.theme().map(|t| (id, t)))
    }

    #[test]
    fn every_stored_number_reads_back_as_its_theme_and_an_unknown_one_as_the_default() {
        for id in ALL {
            assert_eq!(ThemeId::from_u8(id as u8), id);
        }
        assert_eq!(ThemeId::from_u8(200), ThemeId::Storme);
        assert!(ThemeId::Storme.theme().is_none(), "the default is the skin's own");
        assert_eq!(built_in().count(), ALL.len() - 1);
    }

    #[test]
    fn every_theme_reads_well_on_its_panels() {
        let mut all: Vec<String> = Vec::new();
        for (id, t) in built_in() {
            let (panel, field, bar) = (hex(&t.panel), hex(&t.field), hex(&t.bar));
            let name = format!("{id:?}");
            let mut failures = Vec::new();
            for (what, text, ground, min) in [
                ("ink on the panel", hex(&t.ink), panel, 4.5),
                ("ink on the fields", hex(&t.ink), field, 4.5),
                ("ink on the bars", hex(&t.ink), bar, 4.5),
                ("muted text on the panel", hex(&t.muted), panel, 4.5),
                ("dim text on the panel", hex(&t.muted_dim), panel, 3.0),
                ("the accent on the panel", hex(&t.violet), panel, 3.0),
                ("the links on the panel", hex(&t.teal), panel, 4.5),
                ("the text on the accent", hex(&t.on_accent), hex(&t.violet), 3.0),
                ("the text on the pressed accent", hex(&t.on_accent), hex(&t.indigo), 3.0),
                ("danger on the panel", hex(&t.danger), panel, 3.0),
                ("success on the panel", hex(&t.success), panel, 3.0),
                ("warn on the panel", hex(&t.warn), panel, 3.0),
                ("amber on the panel", hex(&t.amber), panel, 3.0),
            ] {
                let ratio = contrast(text, ground);
                if ratio < min {
                    failures.push(format!("{name}: {what} reads {ratio:.2}:1, wants {min}:1"));
                }
            }
            all.extend(failures);
        }
        assert!(all.is_empty(), "{}", all.join("
"));
    }

    #[test]
    fn a_theme_reads_back_through_the_json_a_skin_writes() {
        for (_, t) in built_in() {
            let json = serde_json::to_vec(&t).unwrap();
            let back: Theme = serde_json::from_slice(&json).unwrap();
            assert_eq!(serde_json::to_value(&t).unwrap(), serde_json::to_value(&back).unwrap());
        }
    }

    #[test]
    fn switching_themes_changes_the_palette() {
        let storme = Theme::default().skin_palette();
        for (id, t) in built_in() {
            let p = t.skin_palette();
            assert_ne!(p.panel, storme.panel, "{id:?} must look different from the default");
        }
    }

    #[test]
    fn the_glass_is_a_clamped_share_of_the_panel() {
        // pure: the shared value is left alone, other tests read it
        assert_eq!(scaled(230, glass_percent(1.0)), 230);
        assert_eq!(scaled(200, glass_percent(0.5)), 100);
        assert_eq!(glass_percent(0.0), 35, "never fully clear");
        assert_eq!(glass_percent(f32::NAN), 100, "garbage reads as no glass");
        assert_eq!(glass_percent(9.0), 100);
        assert!(!set_glass(1.0), "the default is already 100");
    }

    #[test]
    fn animation_intensity_scales_egui_transitions() {
        let full = egui::Style::default().animation_time;
        assert_eq!(animation_time(1.0), full);
        assert_eq!(animation_time(0.0), 0.0);
        assert!((animation_time(0.5) - full / 2.0).abs() < 1e-6);
        assert_eq!(animation_time(f32::NAN), full);
        assert_eq!(animation_time(7.0), full);
    }
}
