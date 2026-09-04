//! Windows virtual-desktop capture: monitor geometry, composition and crop.
//!
//! Backend-only P2 slice (ROADMAP-088). Captures the full virtual desktop from
//! per-monitor screenshots and crops a selection rectangle from the saved
//! frame. Nothing calls it yet — the future consumer is the OCR capture command
//! flow (capture → selection overlay → `OcrRuntime::recognize`).
#![allow(dead_code)]

use image::{RgbImage, Rgba, RgbaImage};
use thiserror::Error;

/// Minimum accepted selection side in physical pixels.
pub const MIN_SELECTION_SIDE: u32 = 8;

/// A monitor's position and size in virtual-screen physical pixels.
///
/// Coordinates are relative to the virtual desktop, so `x`/`y` may be negative
/// for monitors left of or above the primary monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Union of all monitors in virtual-screen physical coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualScreenGeometry {
    /// Top-left corner of the bounding union (may be negative).
    pub origin: (i32, i32),
    /// Width and height of the bounding union.
    pub size: (u32, u32),
    /// The monitors that produced this geometry, in enumeration order.
    pub monitors: Vec<MonitorRect>,
}

/// A normalized, axis-aligned selection rectangle in virtual-screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// App-owned capture failure categories.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// The platform reported no monitors.
    #[error("no monitors found")]
    NoMonitors,
    /// Monitor enumeration or capture failed.
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    /// The selection is smaller than [`MIN_SELECTION_SIDE`] on at least one side.
    #[error("selection is too small")]
    SelectionTooSmall,
}

/// Compute the bounding union of the given monitors.
///
/// Origins may be negative; the union is the smallest axis-aligned rectangle
/// covering every monitor. An empty input yields [`CaptureError::NoMonitors`].
pub fn virtual_screen_geometry(
    monitors: &[MonitorRect],
) -> Result<VirtualScreenGeometry, CaptureError> {
    let first = monitors.first().ok_or(CaptureError::NoMonitors)?;

    let mut min_x = first.x as i64;
    let mut min_y = first.y as i64;
    let mut max_x = first.x as i64 + first.width as i64;
    let mut max_y = first.y as i64 + first.height as i64;

    for monitor in &monitors[1..] {
        min_x = min_x.min(monitor.x as i64);
        min_y = min_y.min(monitor.y as i64);
        max_x = max_x.max(monitor.x as i64 + monitor.width as i64);
        max_y = max_y.max(monitor.y as i64 + monitor.height as i64);
    }

    Ok(VirtualScreenGeometry {
        origin: (min_x as i32, min_y as i32),
        size: ((max_x - min_x) as u32, (max_y - min_y) as u32),
        monitors: monitors.to_vec(),
    })
}

/// Normalize an arbitrary two-corner drag into a top-left-based rectangle.
///
/// Independent of drag direction; width and height are `max - min`, floored at
/// zero (a degenerate point produces a zero-size rectangle).
pub fn normalize_selection(x1: i32, y1: i32, x2: i32, y2: i32) -> SelectionRect {
    let x = x1.min(x2);
    let y = y1.min(y2);
    SelectionRect {
        x,
        y,
        width: x1.max(x2).saturating_sub(x) as u32,
        height: y1.max(y2).saturating_sub(y) as u32,
    }
}

/// Intersect a selection with the virtual-screen bounds.
///
/// Returns `None` when the intersection is empty (including a zero-size
/// selection or one entirely outside the virtual desktop).
pub fn clamp_selection(
    rect: SelectionRect,
    geometry: &VirtualScreenGeometry,
) -> Option<SelectionRect> {
    let (ox, oy) = geometry.origin;
    let (ow, oh) = geometry.size;

    let bx = ox as i64 + ow as i64;
    let by = oy as i64 + oh as i64;

    let x = rect.x.max(ox) as i64;
    let y = rect.y.max(oy) as i64;
    let x2 = (rect.x as i64 + rect.width as i64).min(bx);
    let y2 = (rect.y as i64 + rect.height as i64).min(by);

    if x2 <= x || y2 <= y {
        return None;
    }

    Some(SelectionRect {
        x: x as i32,
        y: y as i32,
        width: (x2 - x) as u32,
        height: (y2 - y) as u32,
    })
}

/// Reject selections with a side smaller than [`MIN_SELECTION_SIDE`].
pub fn validate_selection(rect: SelectionRect) -> Result<SelectionRect, CaptureError> {
    if rect.width < MIN_SELECTION_SIDE || rect.height < MIN_SELECTION_SIDE {
        return Err(CaptureError::SelectionTooSmall);
    }
    Ok(rect)
}

/// Compose per-monitor images into one frame in virtual-screen coordinates.
///
/// Each monitor image is painted at its `(x - origin.x, y - origin.y)` offset
/// onto an opaque black frame of `geometry.size`; uncovered areas stay black.
pub fn compose_virtual_frame(
    images: &[(MonitorRect, RgbaImage)],
    geometry: &VirtualScreenGeometry,
) -> RgbaImage {
    let (width, height) = geometry.size;
    let mut frame = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));

    for (monitor, image) in images {
        let dx = (monitor.x - geometry.origin.0) as i64;
        let dy = (monitor.y - geometry.origin.1) as i64;
        image::imageops::overlay(&mut frame, image, dx, dy);
    }

    frame
}

/// Crop `rect` (virtual-screen coordinates) from the composed frame as RGB.
///
/// The crop is converted from RGBA to RGB by dropping the alpha channel.
pub fn crop_to_rgb(
    frame: &RgbaImage,
    geometry: &VirtualScreenGeometry,
    rect: SelectionRect,
) -> RgbImage {
    let dx = (rect.x - geometry.origin.0) as u32;
    let dy = (rect.y - geometry.origin.1) as u32;

    let cropped = image::imageops::crop_imm(frame, dx, dy, rect.width, rect.height).to_image();
    image::DynamicImage::ImageRgba8(cropped).to_rgb8()
}

/// Capture every monitor and compose one full virtual-desktop frame.
///
/// The only IO in this module. A single failed monitor capture fails the whole
/// call — no partial frames are returned.
pub fn capture_virtual_desktop() -> Result<(VirtualScreenGeometry, RgbaImage), CaptureError> {
    let monitors = xcap::Monitor::all().map_err(capture_err)?;

    let mut captures = Vec::with_capacity(monitors.len());
    for monitor in &monitors {
        let rect = MonitorRect {
            x: monitor.x().map_err(capture_err)?,
            y: monitor.y().map_err(capture_err)?,
            width: monitor.width().map_err(capture_err)?,
            height: monitor.height().map_err(capture_err)?,
        };
        let image = monitor.capture_image().map_err(capture_err)?;
        captures.push((rect, image));
    }

    let rects: Vec<MonitorRect> = captures.iter().map(|(rect, _)| *rect).collect();
    let geometry = virtual_screen_geometry(&rects)?;
    let frame = compose_virtual_frame(&captures, &geometry);

    Ok((geometry, frame))
}

fn capture_err(error: xcap::XCapError) -> CaptureError {
    CaptureError::CaptureFailed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> MonitorRect {
        MonitorRect {
            x,
            y,
            width,
            height,
        }
    }

    fn selection(x: i32, y: i32, width: u32, height: u32) -> SelectionRect {
        SelectionRect {
            x,
            y,
            width,
            height,
        }
    }

    fn solid(width: u32, height: u32, color: Rgba<u8>) -> RgbaImage {
        RgbaImage::from_pixel(width, height, color)
    }

    const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
    const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);
    const GREEN: Rgba<u8> = Rgba([0, 255, 0, 255]);
    const BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);

    #[test]
    fn union_with_negative_origins_and_gaps() {
        let monitors = vec![
            rect(-1920, 0, 1920, 1080),
            rect(0, 0, 2560, 1440),
            rect(2560, -200, 1920, 1080),
        ];

        let geometry = virtual_screen_geometry(&monitors).unwrap();

        assert_eq!(geometry.origin, (-1920, -200));
        assert_eq!(geometry.size, (6400, 1640));
        assert_eq!(geometry.monitors, monitors);
    }

    #[test]
    fn union_single_monitor_is_identity() {
        let geometry = virtual_screen_geometry(&[rect(10, -5, 800, 600)]).unwrap();

        assert_eq!(geometry.origin, (10, -5));
        assert_eq!(geometry.size, (800, 600));
    }

    #[test]
    fn union_empty_is_no_monitors() {
        assert!(matches!(
            virtual_screen_geometry(&[]),
            Err(CaptureError::NoMonitors)
        ));
    }

    #[test]
    fn normalize_is_order_independent_in_all_directions() {
        let expected = selection(0, 0, 100, 50);

        assert_eq!(normalize_selection(0, 0, 100, 50), expected);
        assert_eq!(normalize_selection(100, 50, 0, 0), expected);
        assert_eq!(normalize_selection(0, 50, 100, 0), expected);
        assert_eq!(normalize_selection(100, 0, 0, 50), expected);
    }

    #[test]
    fn normalize_negative_coordinates_are_kept() {
        assert_eq!(
            normalize_selection(-30, -40, -10, -20),
            selection(-30, -40, 20, 20)
        );
    }

    #[test]
    fn normalize_degenerate_point_is_zero_size() {
        assert_eq!(normalize_selection(5, 5, 5, 5), selection(5, 5, 0, 0));
    }

    fn test_geometry() -> VirtualScreenGeometry {
        virtual_screen_geometry(&[rect(-1920, -200, 1920, 1080), rect(0, 0, 2560, 1440)]).unwrap()
    }

    #[test]
    fn clamp_fully_inside_is_unchanged() {
        let geometry = test_geometry();
        let rect = selection(0, 0, 100, 100);

        assert_eq!(clamp_selection(rect, &geometry), Some(rect));
    }

    #[test]
    fn clamp_partially_outside_is_intersected() {
        let geometry = test_geometry();
        // geometry bounds: x in [-1920, 2560), y in [-200, 1440)
        let rect = selection(2500, 1400, 200, 200);

        assert_eq!(
            clamp_selection(rect, &geometry),
            Some(selection(2500, 1400, 60, 40))
        );
    }

    #[test]
    fn clamp_fully_outside_is_none() {
        let geometry = test_geometry();

        assert_eq!(clamp_selection(selection(5000, 0, 10, 10), &geometry), None);
        assert_eq!(
            clamp_selection(selection(-5000, 0, 10, 10), &geometry),
            None
        );
    }

    #[test]
    fn clamp_negative_origin_is_handled() {
        let geometry = test_geometry();
        let rect = selection(-2000, -300, 200, 200);

        assert_eq!(
            clamp_selection(rect, &geometry),
            Some(selection(-1920, -200, 120, 100))
        );
    }

    #[test]
    fn clamp_zero_size_selection_is_none() {
        let geometry = test_geometry();
        assert_eq!(clamp_selection(selection(0, 0, 0, 0), &geometry), None);
    }

    #[test]
    fn validate_rejects_side_below_minimum() {
        assert!(matches!(
            validate_selection(selection(0, 0, 7, 20)),
            Err(CaptureError::SelectionTooSmall)
        ));
        assert!(matches!(
            validate_selection(selection(0, 0, 20, 7)),
            Err(CaptureError::SelectionTooSmall)
        ));
        assert!(matches!(
            validate_selection(selection(0, 0, 7, 7)),
            Err(CaptureError::SelectionTooSmall)
        ));
    }

    #[test]
    fn validate_accepts_side_at_minimum() {
        let rect = selection(0, 0, 8, 8);
        assert_eq!(validate_selection(rect).unwrap(), rect);
    }

    #[test]
    fn compose_paints_monitors_at_offsets_with_black_gaps() {
        let geometry = VirtualScreenGeometry {
            origin: (0, 0),
            size: (4, 1),
            monitors: vec![rect(0, 0, 2, 1), rect(3, 0, 1, 1)],
        };

        let mut image_a = RgbaImage::new(2, 1);
        image_a.put_pixel(0, 0, RED);
        image_a.put_pixel(1, 0, BLUE);
        let image_b = solid(1, 1, GREEN);

        let captures = vec![(rect(0, 0, 2, 1), image_a), (rect(3, 0, 1, 1), image_b)];

        let frame = compose_virtual_frame(&captures, &geometry);

        assert_eq!(frame.dimensions(), (4, 1));
        assert_eq!(frame.get_pixel(0, 0), &RED);
        assert_eq!(frame.get_pixel(1, 0), &BLUE);
        assert_eq!(frame.get_pixel(2, 0), &BLACK);
        assert_eq!(frame.get_pixel(3, 0), &GREEN);
    }

    #[test]
    fn compose_respects_negative_origin_offset() {
        let geometry = VirtualScreenGeometry {
            origin: (-2, 0),
            size: (3, 1),
            monitors: vec![rect(-2, 0, 2, 1)],
        };

        let captures = vec![(rect(-2, 0, 2, 1), solid(2, 1, GREEN))];

        let frame = compose_virtual_frame(&captures, &geometry);

        assert_eq!(frame.dimensions(), (3, 1));
        assert_eq!(frame.get_pixel(0, 0), &GREEN);
        assert_eq!(frame.get_pixel(1, 0), &GREEN);
        assert_eq!(frame.get_pixel(2, 0), &BLACK);
    }

    #[test]
    fn crop_returns_correct_region() {
        let geometry = VirtualScreenGeometry {
            origin: (0, 0),
            size: (4, 4),
            monitors: vec![rect(0, 0, 4, 4)],
        };

        let mut frame = solid(4, 4, BLACK);
        frame.put_pixel(1, 1, RED);
        frame.put_pixel(2, 1, GREEN);
        frame.put_pixel(1, 2, BLUE);
        frame.put_pixel(2, 2, RED);

        let cropped = crop_to_rgb(&frame, &geometry, selection(1, 1, 2, 2));

        assert_eq!(cropped.dimensions(), (2, 2));
        assert_eq!(cropped.get_pixel(0, 0), &image::Rgb([255, 0, 0]));
        assert_eq!(cropped.get_pixel(1, 0), &image::Rgb([0, 255, 0]));
        assert_eq!(cropped.get_pixel(0, 1), &image::Rgb([0, 0, 255]));
        assert_eq!(cropped.get_pixel(1, 1), &image::Rgb([255, 0, 0]));
    }

    #[test]
    fn crop_drops_alpha_in_rgba_to_rgb_conversion() {
        let geometry = VirtualScreenGeometry {
            origin: (0, 0),
            size: (1, 1),
            monitors: vec![rect(0, 0, 1, 1)],
        };

        // Alpha is intentionally 0: conversion must drop it, not blend.
        let frame = solid(1, 1, Rgba([10, 20, 30, 0]));

        let cropped = crop_to_rgb(&frame, &geometry, selection(0, 0, 1, 1));

        assert_eq!(cropped.get_pixel(0, 0), &image::Rgb([10, 20, 30]));
    }

    #[test]
    fn crop_across_negative_x_origin() {
        let geometry = VirtualScreenGeometry {
            origin: (-2, 0),
            size: (4, 2),
            monitors: vec![rect(-2, 0, 2, 2)],
        };

        let mut frame = solid(4, 2, BLACK);
        // Frame pixel (1, 0) corresponds to virtual (-1, 0).
        frame.put_pixel(1, 0, RED);

        let cropped = crop_to_rgb(&frame, &geometry, selection(-1, 0, 2, 1));

        assert_eq!(cropped.dimensions(), (2, 1));
        assert_eq!(cropped.get_pixel(0, 0), &image::Rgb([255, 0, 0]));
        assert_eq!(cropped.get_pixel(1, 0), &image::Rgb([0, 0, 0]));
    }

    /// Real-machine smoke of the xcap path. Excluded from standard runs:
    /// captures the actual desktop, requires an interactive session.
    #[test]
    #[ignore]
    fn smoke_capture_real_monitors() {
        let (geometry, frame) = capture_virtual_desktop().expect("real capture");

        assert!(!geometry.monitors.is_empty());
        assert_eq!(
            frame.dimensions(),
            geometry.size,
            "composed frame must match virtual-screen size"
        );

        let corners = [
            normalize_selection(geometry.origin.0, geometry.origin.1, 10, 10),
            normalize_selection(0, 0, 10, 10),
        ];
        for corner in corners {
            if let Some(clamped) = clamp_selection(corner, &geometry) {
                if let Ok(valid) = validate_selection(clamped) {
                    let crop = crop_to_rgb(&frame, &geometry, valid);
                    assert!(crop.dimensions().0 > 0 && crop.dimensions().1 > 0);
                }
            }
        }
    }
}
