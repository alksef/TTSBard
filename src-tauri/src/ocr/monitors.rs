//! Monitor inventory and capture-target primitives (ROADMAP-126).
//!
//! Enumerates the currently attached displays without capturing pixels and
//! resolves a [`CaptureTarget`] against that inventory. On Windows — the
//! product target — each display's persistent device path (from
//! `QueryDisplayConfig` + `DisplayConfigGetDeviceInfo`) is mapped to the GDI
//! source name that [`xcap::Monitor::name`] reports, so a saved target survives
//! list reordering and reconnects; only the device path is ever persisted.

use serde::{Deserialize, Serialize};

use crate::ocr::capture::{CaptureError, MonitorRect};

/// What a one-shot OCR run should capture.
///
/// Internally tagged by `type`: `{"type":"all"}`, `{"type":"primary"}` and
/// `{"type":"monitor","devicePath":"..."}`. The `devicePath` is the persistent
/// Windows device path of the selected display — never an xcap id, `HMONITOR`
/// handle or list index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CaptureTarget {
    All,
    Primary,
    Monitor {
        #[serde(rename = "devicePath")]
        device_path: String,
    },
}

impl Default for CaptureTarget {
    fn default() -> Self {
        CaptureTarget::All
    }
}

/// Serializable description of one selectable display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    /// Persistent Windows device path — the selection key.
    pub device_path: String,
    /// Human label: the friendly device name, falling back to the source name.
    pub label: String,
    /// GDI source name, equal to the value [`xcap::Monitor::name`] reports.
    pub source_name: String,
    /// Whether this display is the primary display.
    pub is_primary: bool,
    /// Position and size in virtual-screen physical pixels.
    pub geometry: MonitorRect,
}

/// Resolution of a [`CaptureTarget`] against the current inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTarget {
    All,
    Single { source_name: String },
}

/// Resolve a capture target against the inventory.
///
/// Pure: no IO, no pixels. `All` maps to the whole virtual desktop; `Primary`
/// resolves the freshly enumerated primary display; a specific `Monitor`
/// resolves by device path (never list index). A missing specific device path
/// yields [`CaptureError::MonitorUnavailable`] with no fallback to another
/// screen.
pub fn resolve_target(
    target: &CaptureTarget,
    inventory: &[MonitorInfo],
) -> Result<ResolvedTarget, CaptureError> {
    match target {
        CaptureTarget::All => Ok(ResolvedTarget::All),
        CaptureTarget::Primary => {
            let info = inventory
                .iter()
                .find(|m| m.is_primary)
                .ok_or(CaptureError::NoMonitors)?;
            Ok(ResolvedTarget::Single {
                source_name: info.source_name.clone(),
            })
        }
        CaptureTarget::Monitor { device_path } => {
            let info = inventory
                .iter()
                .find(|m| &m.device_path == device_path)
                .ok_or(CaptureError::MonitorUnavailable)?;
            Ok(ResolvedTarget::Single {
                source_name: info.source_name.clone(),
            })
        }
    }
}

/// Enumerate all currently attached displays without capturing pixels.
///
/// Any failure — including an unexpectedly empty result — is an error; the
/// caller must not fall back to a partial or guessed list.
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, CaptureError> {
    let monitors = xcap::Monitor::all().map_err(capture_err)?;
    inventory_for_monitors(&monitors)
}

/// Metadata for the handles acquired by this request. Targeted capture uses
/// these same handles, rather than reacquiring potentially reassigned sources.
pub(crate) fn inventory_for_monitors(
    monitors: &[xcap::Monitor],
) -> Result<Vec<MonitorInfo>, CaptureError> {
    enumerate_native(monitors)
}

fn capture_err(error: xcap::XCapError) -> CaptureError {
    CaptureError::CaptureFailed(error.to_string())
}

#[cfg(not(windows))]
fn enumerate_native(monitors: &[xcap::Monitor]) -> Result<Vec<MonitorInfo>, CaptureError> {
    if monitors.is_empty() {
        return Err(CaptureError::NoMonitors);
    }

    let mut inventory = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let source_name = monitor.name().map_err(capture_err)?;
        let geometry = MonitorRect {
            x: monitor.x().map_err(capture_err)?,
            y: monitor.y().map_err(capture_err)?,
            width: monitor.width().map_err(capture_err)?,
            height: monitor.height().map_err(capture_err)?,
        };
        let is_primary = monitor.is_primary().map_err(capture_err)?;
        // Non-Windows has no persistent device path: the source name is the
        // platform-local fallback selection key.
        inventory.push(MonitorInfo {
            device_path: source_name.clone(),
            label: source_name.clone(),
            source_name,
            is_primary,
            geometry,
        });
    }
    Ok(inventory)
}

#[cfg(windows)]
fn enumerate_native(monitors: &[xcap::Monitor]) -> Result<Vec<MonitorInfo>, CaptureError> {
    if monitors.is_empty() {
        return Err(CaptureError::NoMonitors);
    }

    let mut gdi_infos = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let source_name = monitor.name().map_err(capture_err)?;
        let geometry = MonitorRect {
            x: monitor.x().map_err(capture_err)?,
            y: monitor.y().map_err(capture_err)?,
            width: monitor.width().map_err(capture_err)?,
            height: monitor.height().map_err(capture_err)?,
        };
        let is_primary = monitor.is_primary().map_err(capture_err)?;
        gdi_infos.push(GdiMonitorInfo {
            source_name,
            geometry,
            is_primary,
        });
    }

    let targets = query_active_targets()?;
    let sources: Vec<&str> = gdi_infos
        .iter()
        .map(|info| info.source_name.as_str())
        .collect();
    let target_sources: Vec<(&str, &str)> = targets
        .iter()
        .map(|target| (target.source_name.as_str(), target.device_path.as_str()))
        .collect();
    validate_inventory_sources(&sources, &target_sources)?;

    let mut inventory = Vec::with_capacity(targets.len());
    for target in &targets {
        let gdi = gdi_infos
            .iter()
            .find(|g| g.source_name == target.source_name)
            .ok_or_else(|| {
                CaptureError::CaptureFailed(format!(
                    "active display target {} has no matching monitor source",
                    target.device_path
                ))
            })?;
        let label = if target.friendly_name.is_empty() {
            target.source_name.clone()
        } else {
            target.friendly_name.clone()
        };
        inventory.push(MonitorInfo {
            device_path: target.device_path.clone(),
            label,
            source_name: target.source_name.clone(),
            is_primary: gdi.is_primary,
            geometry: gdi.geometry,
        });
    }

    if inventory.is_empty() {
        return Err(CaptureError::NoMonitors);
    }

    Ok(inventory)
}

/// Reject inconsistent topology snapshots; clone targets may share a source,
/// but every source must be covered and every selectable device key unique.
#[cfg(any(windows, test))]
fn validate_inventory_sources(
    sources: &[&str],
    targets: &[(&str, &str)],
) -> Result<(), CaptureError> {
    let mut keys = std::collections::HashSet::new();
    if targets.iter().any(|(source, key)| {
        key.trim().is_empty() || !keys.insert(*key) || !sources.contains(source)
    }) || sources
        .iter()
        .any(|source| !targets.iter().any(|(target, _)| target == source))
    {
        return Err(CaptureError::CaptureFailed(
            "inconsistent monitor inventory".to_string(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
struct GdiMonitorInfo {
    source_name: String,
    geometry: MonitorRect,
    is_primary: bool,
}

#[cfg(windows)]
struct DisplayTarget {
    source_name: String,
    device_path: String,
    friendly_name: String,
}

#[cfg(windows)]
const MAX_DISPLAY_CONFIG_RETRIES: usize = 3;

#[cfg(windows)]
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LUID};

/// Query the active display paths and resolve their source/target names.
///
/// `GetDisplayConfigBufferSizes` / `QueryDisplayConfig` buffer sizes can change
/// between calls (a monitor is plugged or unplugged), so the pair is retried a
/// bounded number of times when the fill reports an insufficient buffer.
#[cfg(windows)]
fn query_active_targets() -> Result<Vec<DisplayTarget>, CaptureError> {
    for _ in 0..MAX_DISPLAY_CONFIG_RETRIES {
        let mut num_paths: u32 = 0;
        let mut num_modes: u32 = 0;

        let size_status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes)
        };
        if size_status != ERROR_SUCCESS {
            return Err(CaptureError::CaptureFailed(format!(
                "GetDisplayConfigBufferSizes failed: {size_status:?}"
            )));
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); num_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); num_modes as usize];

        let status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut num_paths,
                paths.as_mut_ptr(),
                &mut num_modes,
                modes.as_mut_ptr(),
                None,
            )
        };

        if status == ERROR_INSUFFICIENT_BUFFER {
            // The topology changed between the size query and the fill:
            // reacquire fresh sizes and retry (bounded).
            continue;
        }
        if status != ERROR_SUCCESS {
            return Err(CaptureError::CaptureFailed(format!(
                "QueryDisplayConfig failed: {status:?}"
            )));
        }

        paths.truncate(num_paths as usize);

        let mut targets = Vec::with_capacity(paths.len());
        for path in &paths {
            let source_name = source_device_name(path.sourceInfo.adapterId, path.sourceInfo.id)?;
            let (device_path, friendly_name) =
                target_device_name(path.targetInfo.adapterId, path.targetInfo.id)?;
            targets.push(DisplayTarget {
                source_name,
                device_path,
                friendly_name,
            });
        }

        return Ok(targets);
    }

    Err(CaptureError::CaptureFailed(format!(
        "QueryDisplayConfig failed after {MAX_DISPLAY_CONFIG_RETRIES} buffer retries"
    )))
}

#[cfg(windows)]
fn source_device_name(adapter: LUID, id: u32) -> Result<String, CaptureError> {
    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id,
        },
        ..Default::default()
    };

    let status = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
    if status != 0 {
        return Err(CaptureError::CaptureFailed(format!(
            "DisplayConfigGetDeviceInfo(source) failed with code {status}"
        )));
    }

    Ok(fixed_u16_to_string(&source.viewGdiDeviceName))
}

#[cfg(windows)]
fn target_device_name(adapter: LUID, id: u32) -> Result<(String, String), CaptureError> {
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id,
        },
        ..Default::default()
    };

    let status = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) };
    if status != 0 {
        return Err(CaptureError::CaptureFailed(format!(
            "DisplayConfigGetDeviceInfo(target) failed with code {status}"
        )));
    }

    Ok((
        fixed_u16_to_string(&target.monitorDevicePath),
        fixed_u16_to_string(&target.monitorFriendlyDeviceName),
    ))
}

/// Decode a NUL-terminated UTF-16 fixed buffer into a `String`.
#[cfg(windows)]
fn fixed_u16_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_correspondence_rejects_incomplete_and_invalid_keys() {
        assert!(
            validate_inventory_sources(&["display1", "display2"], &[("display1", "device1")])
                .is_err()
        );
        assert!(validate_inventory_sources(&["display1"], &[("other", "device1")]).is_err());
        assert!(validate_inventory_sources(&["display1"], &[("display1", "")]).is_err());
        assert!(validate_inventory_sources(
            &["display1"],
            &[("display1", "device1"), ("display1", "device1")]
        )
        .is_err());
        assert!(validate_inventory_sources(
            &["display1"],
            &[("display1", "device1"), ("display1", "device2")]
        )
        .is_ok());
    }

    fn info(device_path: &str, source_name: &str, is_primary: bool) -> MonitorInfo {
        MonitorInfo {
            device_path: device_path.to_string(),
            label: format!("Label {source_name}"),
            source_name: source_name.to_string(),
            is_primary,
            geometry: MonitorRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    fn sample_inventory() -> Vec<MonitorInfo> {
        vec![
            info(r"\\.\DISPLAY1\Monitor0", r"\\.\DISPLAY1", true),
            info(r"\\.\DISPLAY2\Monitor0", r"\\.\DISPLAY2", false),
        ]
    }

    #[test]
    fn default_target_is_all() {
        assert_eq!(CaptureTarget::default(), CaptureTarget::All);
    }

    #[test]
    fn target_serde_roundtrips_all_variants() {
        let variants = [
            CaptureTarget::All,
            CaptureTarget::Primary,
            CaptureTarget::Monitor {
                device_path: r"\\.\DISPLAY1\Monitor0".to_string(),
            },
        ];
        for target in variants {
            let json = serde_json::to_string(&target).unwrap();
            let back: CaptureTarget = serde_json::from_str(&json).unwrap();
            assert_eq!(back, target);
        }
    }

    #[test]
    fn target_serde_emits_exact_wire_shape() {
        assert_eq!(
            serde_json::to_value(CaptureTarget::All).unwrap(),
            serde_json::json!({ "type": "all" })
        );
        assert_eq!(
            serde_json::to_value(CaptureTarget::Primary).unwrap(),
            serde_json::json!({ "type": "primary" })
        );
        assert_eq!(
            serde_json::to_value(CaptureTarget::Monitor {
                device_path: r"\\.\DISPLAY1\Monitor0".to_string(),
            })
            .unwrap(),
            serde_json::json!({ "type": "monitor", "devicePath": r"\\.\DISPLAY1\Monitor0" })
        );
    }

    #[test]
    fn resolve_all_ignores_inventory() {
        assert_eq!(
            resolve_target(&CaptureTarget::All, &[]).unwrap(),
            ResolvedTarget::All
        );
    }

    #[test]
    fn resolve_primary_picks_primary_monitor() {
        let result = resolve_target(&CaptureTarget::Primary, &sample_inventory()).unwrap();
        assert_eq!(
            result,
            ResolvedTarget::Single {
                source_name: r"\\.\DISPLAY1".to_string()
            }
        );
    }

    #[test]
    fn resolve_specific_matches_device_path() {
        let result = resolve_target(
            &CaptureTarget::Monitor {
                device_path: r"\\.\DISPLAY2\Monitor0".to_string(),
            },
            &sample_inventory(),
        )
        .unwrap();
        assert_eq!(
            result,
            ResolvedTarget::Single {
                source_name: r"\\.\DISPLAY2".to_string()
            }
        );
    }

    #[test]
    fn resolve_missing_target_is_monitor_unavailable() {
        assert!(matches!(
            resolve_target(
                &CaptureTarget::Monitor {
                    device_path: r"\\.\DISPLAY3\Monitor0".to_string()
                },
                &sample_inventory(),
            ),
            Err(CaptureError::MonitorUnavailable)
        ));
    }

    #[test]
    fn resolve_specific_is_index_independent() {
        let mut reordered = sample_inventory();
        reordered.swap(0, 1);

        let target = CaptureTarget::Monitor {
            device_path: r"\\.\DISPLAY1\Monitor0".to_string(),
        };
        let a = resolve_target(&target, &sample_inventory()).unwrap();
        let b = resolve_target(&target, &reordered).unwrap();

        assert_eq!(a, b);
        assert_eq!(
            a,
            ResolvedTarget::Single {
                source_name: r"\\.\DISPLAY1".to_string()
            }
        );
    }

    #[test]
    fn resolve_primary_with_empty_inventory_is_no_monitors() {
        assert!(matches!(
            resolve_target(&CaptureTarget::Primary, &[]),
            Err(CaptureError::NoMonitors)
        ));
    }
}
