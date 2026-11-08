//! Deterministic wallpaper artwork for catalogue entries.
//!
//! Catalogue entries are art directions (a palette plus an effect motif),
//! not files. This module renders each one into a real wallpaper: a moody
//! diagonal gradient built from the entry's placeholder colours, the motif
//! (orbit rings, sweeping ribbons or a fine grid) and a soft vignette.
//! Everything is seeded by the entry id, so an entry always renders exactly
//! the same image — and the library de-duplicates by content hash.

use image::{Rgba, RgbaImage};

use crate::catalogue::CatalogueEntry;

fn parse_hex(value: &str) -> Option<[f32; 3]> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32;
    Some([r, g, b])
}

fn seed_of(id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash | 1
}

struct TinyRng(u64);

impl TinyRng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// A soft glow: normalised position, radius and intensity.
struct Glow {
    x: f32,
    y: f32,
    radius: f32,
    intensity: f32,
}

pub fn render_entry(entry: &CatalogueEntry, width: u32, height: u32) -> RgbaImage {
    let dominant =
        parse_hex(&entry.placeholder.dominant_hex).unwrap_or([90.0, 140.0, 230.0]);
    let accent = parse_hex(&entry.placeholder.accent_hex).unwrap_or([160.0, 210.0, 255.0]);
    let accent_light = mix(accent, [255.0, 255.0, 255.0], 0.35);
    let dark = [dominant[0] * 0.10, dominant[1] * 0.12, dominant[2] * 0.16];

    let style = entry
        .effect
        .as_ref()
        .map(|effect| effect.style.as_str())
        .unwrap_or("");
    let mut rng = TinyRng(seed_of(&entry.id));

    // Motif setup.
    let mut glows: Vec<Glow> = Vec::new();
    // (slope, intercept, half-width, intensity) in normalised space.
    let mut ribbons: Vec<(f32, f32, f32, f32)> = Vec::new();
    match style {
        "orbit" => {
            let count = 4 + (rng.next() % 3) as usize;
            for _ in 0..count {
                glows.push(Glow {
                    x: 0.12 + rng.unit() * 0.76,
                    y: 0.12 + rng.unit() * 0.76,
                    radius: 0.09 + rng.unit() * 0.17,
                    intensity: 0.22 + rng.unit() * 0.33,
                });
            }
        }
        "ribbon" => {
            let count = 2 + (rng.next() % 2) as usize;
            for _ in 0..count {
                ribbons.push((
                    0.45 + rng.unit() * 0.5,
                    -0.4 + rng.unit() * 1.1,
                    0.06 + rng.unit() * 0.09,
                    0.10 + rng.unit() * 0.16,
                ));
            }
            glows.push(Glow {
                x: 0.30 + rng.unit() * 0.4,
                y: 0.42,
                radius: 0.5,
                intensity: 0.10,
            });
        }
        "grid" => {
            glows.push(Glow {
                x: 0.5,
                y: 0.42,
                radius: 0.42,
                intensity: 0.16,
            });
        }
        _ => {
            glows.push(Glow {
                x: 0.34,
                y: 0.30,
                radius: 0.55,
                intensity: 0.10,
            });
        }
    }

    let grid_cell = (width as f32 / 22.0).max(24.0);
    let aspect = height as f32 / width as f32;

    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;

            // Diagonal gradient: near-black -> dominant -> accent.
            let t = ((u * 0.9 + v * 0.7) * 0.72).clamp(0.0, 1.0);
            let mut color = if t < 0.55 {
                mix(dark, dominant, (t / 0.55).powf(1.15))
            } else {
                mix(dominant, accent, (t - 0.55) / 0.45)
            };

            // Glows (aspect-corrected normalised space).
            for glow in &glows {
                let dx = u - glow.x;
                let dy = (v - glow.y) * aspect;
                let distance = (dx * dx + dy * dy).sqrt();
                let reach = glow.radius * 1.6;
                if distance < reach {
                    let falloff = 1.0 - distance / reach;
                    color = mix(color, accent_light, falloff * falloff * glow.intensity);
                }
            }

            // Ribbons.
            for (slope, intercept, half_width, intensity) in &ribbons {
                let distance = (v - (slope * u + intercept)).abs() / half_width;
                if distance < 1.0 {
                    let falloff = (1.0 - distance).powf(1.6);
                    color = mix(color, accent_light, falloff * intensity);
                }
            }

            // Grid.
            if style == "grid" {
                let fx = x as f32 % grid_cell;
                let fy = y as f32 % grid_cell;
                let line = fx.min(grid_cell - fx).min(fy.min(grid_cell - fy));
                if line < 1.0 {
                    color = mix(color, accent_light, (1.0 - line) * 0.10);
                }
            }

            // Vignette.
            let vx = u * 2.0 - 1.0;
            let vy = v * 2.0 - 1.0;
            let vignette = 1.0 - 0.34 * (vx * vx + vy * vy).min(1.0);
            color = [
                color[0] * vignette,
                color[1] * vignette,
                color[2] * vignette,
            ];

            image.put_pixel(
                x,
                y,
                Rgba([
                    color[0].clamp(0.0, 255.0) as u8,
                    color[1].clamp(0.0, 255.0) as u8,
                    color[2].clamp(0.0, 255.0) as u8,
                    255,
                ]),
            );
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::{EffectSpec, PlaceholderColors, Resolution};

    fn test_entry(style: Option<&str>) -> CatalogueEntry {
        CatalogueEntry {
            id: "test-entry".into(),
            title: "Test".into(),
            description: None,
            media_type: "static".into(),
            game: None,
            tags: vec![],
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            licence_id: "TEST".into(),
            added_at: "2026-01-01".into(),
            placeholder: PlaceholderColors {
                dominant_hex: "#3f7dff".into(),
                accent_hex: "#6fd3ff".into(),
            },
            interactive: true,
            effect: style.map(|style| EffectSpec {
                style: style.into(),
            }),
        }
    }

    #[test]
    fn renders_deterministic_artwork() {
        let entry = test_entry(Some("orbit"));
        let first = render_entry(&entry, 96, 54);
        let second = render_entry(&entry, 96, 54);
        assert_eq!(first.dimensions(), (96, 54));
        assert_eq!(first.as_raw(), second.as_raw(), "renders must be deterministic");

        let origin = first.get_pixel(0, 0).0;
        assert!(
            first.pixels().any(|pixel| pixel.0 != origin),
            "artwork must not be flat"
        );

        for style in ["ribbon", "grid", ""] {
            let variant = test_entry(if style.is_empty() { None } else { Some(style) });
            let image = render_entry(&variant, 64, 36);
            assert_eq!(image.dimensions(), (64, 36));
        }
    }
}
