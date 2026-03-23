use tauri::image::Image;
use tauri::AppHandle;

pub const TRAY_ID: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    NoSessions,
    Active,
    NeedsAttention,
}

/// Pre-computed tray icon variants (raw RGBA buffers).
pub struct TrayIcons {
    base_rgba: Vec<u8>,
    green_rgba: Vec<u8>,
    amber_rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl TrayIcons {
    pub fn new() -> Self {
        let base_image = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
            .expect("Failed to decode tray icon");
        let width = base_image.width();
        let height = base_image.height();
        let base_rgba = base_image.rgba().to_vec();

        // Apple system green / orange — visible on both light and dark backgrounds
        let green_rgba = create_dot_variant(&base_rgba, width, height, [48, 209, 88]);
        let amber_rgba = create_dot_variant(&base_rgba, width, height, [255, 159, 10]);

        Self {
            base_rgba,
            green_rgba,
            amber_rgba,
            width,
            height,
        }
    }
}

/// Create a non-template icon variant with a colored dot in the bottom-right.
///
/// The base template icon (black shape + alpha) is converted to medium gray
/// so it remains visible on both light and dark menu bars when template mode
/// is disabled.
fn create_dot_variant(rgba: &[u8], width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let mut result = rgba.to_vec();

    // Convert template pixels to medium gray for non-template rendering
    for i in (0..result.len()).step_by(4) {
        if result[i + 3] > 0 {
            result[i] = 140;
            result[i + 1] = 140;
            result[i + 2] = 140;
        }
    }

    // Draw filled circle — bottom-right corner
    let r = (width.min(height) as f32 * 0.19).round() as i32;
    let margin = (r as f32 * 0.35).round() as i32;
    let cx = width as i32 - r - margin;
    let cy = height as i32 - r - margin;

    for y in (cy - r - 1).max(0)..=(cy + r + 1).min(height as i32 - 1) {
        for x in (cx - r - 1).max(0)..=(cx + r + 1).min(width as i32 - 1) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r * r {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                result[idx] = color[0];
                result[idx + 1] = color[1];
                result[idx + 2] = color[2];
                result[idx + 3] = 255;
            }
        }
    }

    result
}

static CURRENT_STATUS: std::sync::Mutex<Option<TrayStatus>> = std::sync::Mutex::new(None);

/// Update the tray icon and tooltip based on session status.
/// Only redraws the icon when the status category changes.
pub fn update_tray(app_handle: &AppHandle, icons: &TrayIcons, status: TrayStatus, tooltip: &str) {
    let status_changed = {
        let mut current = CURRENT_STATUS.lock().unwrap();
        if *current == Some(status) {
            false
        } else {
            *current = Some(status);
            true
        }
    };

    let Some(tray) = app_handle.tray_by_id(TRAY_ID) else {
        return;
    };

    if status_changed {
        match status {
            TrayStatus::NoSessions => {
                let icon =
                    Image::new_owned(icons.base_rgba.clone(), icons.width, icons.height);
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(true);
            }
            TrayStatus::Active => {
                let icon =
                    Image::new_owned(icons.green_rgba.clone(), icons.width, icons.height);
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(false);
            }
            TrayStatus::NeedsAttention => {
                let icon =
                    Image::new_owned(icons.amber_rgba.clone(), icons.width, icons.height);
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(false);
            }
        }
    }

    let _ = tray.set_tooltip(Some(tooltip));
}
