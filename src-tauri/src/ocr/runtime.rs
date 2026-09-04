//! PP-OCRv5 runtime adapter backed by `rapidocr-core`.
//!
//! Backend-only P0 slice (ROADMAP-088): the facade owns a dedicated single-owner
//! worker with long-lived detection/recognition ONNX sessions and turns an
//! already-captured RGB image into a normalized reading-order result. No
//! settings, commands, UI, screenshots, or incoming delivery is wired up yet.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use image::RgbImage;
use rapidocr_core::config::{
    DetConfig, DetInputLimits, DetOverflowBehavior, ExecutionProvider, InferenceOptions, LimitType,
    PipelineConfig, RapidOcrConfig, RecConfig,
};
use rapidocr_core::types::TimedOcrOutput;
use rapidocr_core::{TokioOcrError, TokioRapidOcr};
use thiserror::Error;

use super::packs::{OcrFamily, OcrPackDescriptor};

/// Cooperative deadline for a single OCR request, including queue wait time.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Worker request queue capacity. A single in-flight request keeps the bounded
/// worker strictly sequential and backpressures callers immediately.
const WORKER_QUEUE_CAPACITY: usize = 1;

/// App-owned OCR failure categories.
///
/// Keeps third-party `TokioOcrError` out of the crate boundary while retaining
/// enough safe context to distinguish a transient busy queue from a fatal
/// initialization failure.
#[derive(Debug, Error)]
pub enum OcrRuntimeError {
    /// The descriptor does not name a supported model family.
    #[error("unsupported OCR pack family")]
    UnsupportedFamily,
    /// The ONNX pipeline could not be initialized or its models loaded.
    #[error("OCR pipeline initialization failed: {0}")]
    Init(String),
    /// The bounded worker queue is full; the caller may retry later.
    #[error("OCR request queue is full")]
    Busy,
    /// The request was cooperatively cancelled.
    #[error("OCR request was cancelled")]
    Cancelled,
    /// The cooperative timeout elapsed before the worker finished.
    #[error("OCR request timed out after {0:?}")]
    Timeout(Duration),
    /// The dedicated worker is stopped or shutting down.
    #[error("OCR worker stopped")]
    Stopped,
    /// The dedicated worker thread panicked.
    #[error("OCR worker panicked")]
    WorkerPanicked,
    /// OCR inference failed for the supplied image.
    #[error("OCR inference failed")]
    Inference,
}

/// Four-point text box in source image pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrBox {
    /// Box corners as four `[x, y]` points.
    pub points: [[f32; 2]; 4],
}

/// One recognized OCR line with its normalized text.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    /// Detected text box.
    pub bbox: OcrBox,
    /// Whitespace-normalized, trimmed text.
    pub text: String,
    /// Mean recognition confidence.
    pub confidence: f32,
}

/// Normalized OCR result in reading order.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    /// Lines joined with `\n` in library reading order.
    pub text: String,
    /// Recognized lines with boxes and confidence, in reading order.
    pub lines: Vec<OcrLine>,
    /// Total elapsed milliseconds for the request.
    pub total_ms: f64,
}

/// Facade owning a `TokioRapidOcr` worker for a single validated pack.
pub struct OcrRuntime {
    service: TokioRapidOcr,
}

impl OcrRuntime {
    /// Builds and starts the worker for a validated PP-OCRv5 pack.
    ///
    /// Rejects any non-`PpOcrV5` descriptor and uses only the descriptor's
    /// resolved paths. Does not rediscover, download, or substitute models.
    pub async fn new(descriptor: &OcrPackDescriptor) -> Result<Self, OcrRuntimeError> {
        let config = build_config(descriptor)?;
        let service = TokioRapidOcr::new_with_queue_capacity(config, WORKER_QUEUE_CAPACITY)
            .await
            .map_err(map_init_error)?;
        Ok(Self { service })
    }

    /// Runs one OCR request on the dedicated worker with a cooperative timeout.
    ///
    /// The worker owns the ONNX sessions and processes the image off the Tokio
    /// executor; this method only awaits the bounded request without blocking an
    /// executor thread.
    pub async fn recognize(&self, image: Arc<RgbImage>) -> Result<OcrResult, OcrRuntimeError> {
        let timed = self
            .service
            .run_image_with_timeout(image, REQUEST_TIMEOUT)
            .await
            .map_err(map_request_error)?;
        Ok(map_output(timed))
    }

    /// Consumes the facade, requests cooperative shutdown, and joins the worker.
    pub async fn shutdown(self) -> Result<(), OcrRuntimeError> {
        self.service.shutdown().await.map_err(map_request_error)
    }
}

/// Builds the tested PP-OCRv5 CPU configuration from a validated descriptor.
fn build_config(descriptor: &OcrPackDescriptor) -> Result<RapidOcrConfig, OcrRuntimeError> {
    if descriptor.family != OcrFamily::PpOcrV5 {
        return Err(OcrRuntimeError::UnsupportedFamily);
    }

    Ok(RapidOcrConfig {
        pipeline: PipelineConfig::without_cls(),
        inference: InferenceOptions {
            intra_threads: 2,
            inter_threads: 1,
            parallel_execution: false,
            enable_cpu_mem_arena: false,
            execution_provider: ExecutionProvider::Cpu,
        },
        text_score: 0.5,
        min_side_len: 30,
        max_side_len: 2000,
        min_height: 30,
        width_height_ratio: 8.0,
        det: Some(DetConfig {
            model_path: descriptor.det_path.clone(),
            limit_side_len: 736,
            limit_type: LimitType::Min,
            input_limits: DetInputLimits {
                max_side_len: Some(4096),
                max_pixels: Some(4_194_304),
                overflow_behavior: DetOverflowBehavior::Downscale,
            },
            mean: [0.5; 3],
            std: [0.5; 3],
            thresh: 0.3,
            box_thresh: 0.5,
            max_candidates: 1000,
            unclip_ratio: 1.6,
            min_size: 3,
        }),
        cls: None,
        rec: Some(RecConfig {
            model_path: descriptor.rec_path.clone(),
            dict_path: descriptor.dict_path.clone(),
            image_shape: [3, 48, 320],
            batch_size: 6,
        }),
    })
}

/// Maps a runtime worker error to its app-owned category.
///
/// The third-party inference error body is dropped: it may carry raw recognized
/// text or other unsafe content and must not be surfaced in errors or logs.
fn map_request_error(error: TokioOcrError) -> OcrRuntimeError {
    match error {
        TokioOcrError::QueueFull => OcrRuntimeError::Busy,
        TokioOcrError::WorkerStopped => OcrRuntimeError::Stopped,
        TokioOcrError::WorkerPanicked => OcrRuntimeError::WorkerPanicked,
        TokioOcrError::Cancelled => OcrRuntimeError::Cancelled,
        TokioOcrError::TimedOut(duration) => OcrRuntimeError::Timeout(duration),
        TokioOcrError::Ocr(_) => OcrRuntimeError::Inference,
    }
}

/// Maps a construction error, retaining model-load context for initialization.
fn map_init_error(error: TokioOcrError) -> OcrRuntimeError {
    match error {
        TokioOcrError::Ocr(context) => {
            let raw = format!("{context:#}");
            if next_absolute_path_start(&raw).is_some() {
                // Extracting paths from free-form text is heuristic, so a
                // detected absolute path never reaches the frontend verbatim:
                // the UI gets a fixed message, the per-path-masked chain is
                // only traced.
                tracing::error!(
                    detail = %sanitize_init_message(&raw),
                    "OCR runtime init failed (masked absolute paths for UI)"
                );
                OcrRuntimeError::Init(INIT_PATH_MASKED_MESSAGE.to_string())
            } else {
                OcrRuntimeError::Init(raw)
            }
        }
        other => map_request_error(other),
    }
}

/// Fixed user-facing message for init errors that embed absolute paths. The
/// masked detail goes to the tracing log only (see [`map_init_error`]).
const INIT_PATH_MASKED_MESSAGE: &str =
    "OCR model initialization failed (see application log for details)";

/// Masks absolute filesystem paths embedded in an init error message for the
/// tracing log.
///
/// The third-party ONNX load error chain can embed the resolved model/resource
/// paths; every absolute path is replaced through
/// [`crate::secret_log::safe_path_for_log`]. Never shown to the frontend as-is:
/// [`map_init_error`] degrades any path-bearing message to a fixed string
/// first; this masking is the log-side detail.
fn sanitize_init_message(message: &str) -> String {
    let mut sanitized = String::with_capacity(message.len());
    let mut rest = message;

    while let Some(start) = next_absolute_path_start(rest) {
        sanitized.push_str(&rest[..start]);
        let candidate = &rest[start..];
        let end = path_end(candidate);
        sanitized.push_str(&crate::secret_log::safe_path_for_log(Path::new(
            &candidate[..end],
        )));
        rest = &candidate[end..];
    }

    sanitized.push_str(rest);
    sanitized
}

/// Byte length of the path starting at the beginning of `candidate`.
///
/// The path ends at a quote or whitespace-separated token that does not
/// continue the path, so a path containing spaces
/// (`C:\Users\John Doe\AppData\...`) is consumed whole: a token continues the
/// path only when it contains a path separator and does not itself start a
/// new absolute path (drive letter or UNC prefix).
fn path_end(candidate: &str) -> usize {
    let bytes = candidate.as_bytes();
    let mut end = token_end(bytes, 0);
    while let Some(next) = continuation_end(bytes, end) {
        end = next;
    }
    end
}

/// End of the token at `start`: the first quote or ASCII whitespace, or the
/// end of the string.
fn token_end(bytes: &[u8], start: usize) -> usize {
    (start..bytes.len())
        .find(|&i| bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i].is_ascii_whitespace())
        .unwrap_or(bytes.len())
}

/// End of the next path-continuation token, when `bytes[end..]` is whitespace
/// followed by a token that contains a path separator and does not start a new
/// absolute path itself.
fn continuation_end(bytes: &[u8], end: usize) -> Option<usize> {
    let mut start = end;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    if start == end {
        return None;
    }
    let next_end = token_end(bytes, start);
    let token = &bytes[start..next_end];
    if !token.iter().any(|b| *b == b'\\' || *b == b'/') {
        return None;
    }
    let starts_new_path = token.len() > 2
        && ((token[0].is_ascii_alphabetic()
            && token[1] == b':'
            && matches!(token[2], b'\\' | b'/'))
            || (token[0] == b'\\' && token[1] == b'\\'));
    if starts_new_path {
        None
    } else {
        Some(next_end)
    }
}

/// Byte index where an absolute path (drive letter or UNC) begins, or `None`.
fn next_absolute_path_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 2 < bytes.len()
            && bytes[i].is_ascii_alphabetic()
            && bytes[i + 1] == b':'
            && matches!(bytes[i + 2], b'\\' | b'/')
        {
            return Some(i);
        }
        if i + 1 < bytes.len() && bytes[i] == b'\\' && bytes[i + 1] == b'\\' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Collapses Unicode whitespace runs to a single ASCII space and trims ends.
fn normalize_line(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !normalized.is_empty() {
                pending_space = true;
            }
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(ch);
        }
    }
    normalized
}

/// Maps third-party timed output into app-owned result types.
///
/// Preserves library reading order, boxes, scores, and total timing. Empty
/// normalized lines are dropped.
fn map_output(timed: TimedOcrOutput) -> OcrResult {
    let lines: Vec<OcrLine> = timed
        .output
        .lines
        .into_iter()
        .filter_map(|line| {
            let text = normalize_line(&line.text);
            if text.is_empty() {
                return None;
            }
            Some(OcrLine {
                bbox: OcrBox {
                    points: line.bbox.points,
                },
                text,
                confidence: line.score,
            })
        })
        .collect();

    let text = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    OcrResult {
        text,
        lines,
        total_ms: timed.timings.total_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapidocr_core::types::{OcrLine as RapidOcrLine, OcrOutput, OcrTimings, Quad};

    fn descriptor() -> OcrPackDescriptor {
        let root = std::path::PathBuf::from("C:/packs/eslav");
        OcrPackDescriptor {
            id: "eslav".to_string(),
            display_name: "eSlav PP-OCRv5".to_string(),
            languages: vec!["ru".to_string(), "en".to_string()],
            family: OcrFamily::PpOcrV5,
            pack_root: root.clone(),
            det_path: root.join("det.onnx"),
            rec_path: root.join("rec.onnx"),
            dict_path: root.join("dict.txt"),
        }
    }

    #[test]
    fn config_contains_exact_constants_and_descriptor_paths() {
        let root = std::path::PathBuf::from("C:/packs/eslav");
        let cfg = build_config(&descriptor()).unwrap();

        assert_eq!(cfg.pipeline, PipelineConfig::without_cls());
        assert_eq!(cfg.inference.intra_threads, 2);
        assert_eq!(cfg.inference.inter_threads, 1);
        assert!(!cfg.inference.parallel_execution);
        assert!(!cfg.inference.enable_cpu_mem_arena);
        assert_eq!(cfg.inference.execution_provider, ExecutionProvider::Cpu);
        assert_eq!(cfg.text_score, 0.5);
        assert_eq!(cfg.min_side_len, 30);
        assert_eq!(cfg.max_side_len, 2000);
        assert_eq!(cfg.min_height, 30);
        assert_eq!(cfg.width_height_ratio, 8.0);

        let det = cfg.det.as_ref().unwrap();
        assert_eq!(det.model_path, root.join("det.onnx"));
        assert_eq!(det.limit_side_len, 736);
        assert_eq!(det.limit_type, LimitType::Min);
        assert_eq!(det.mean, [0.5; 3]);
        assert_eq!(det.std, [0.5; 3]);
        assert_eq!(det.thresh, 0.3);
        assert_eq!(det.box_thresh, 0.5);
        assert_eq!(det.max_candidates, 1000);
        assert_eq!(det.unclip_ratio, 1.6);
        assert_eq!(det.min_size, 3);
        assert_eq!(det.input_limits.max_side_len, Some(4096));
        assert_eq!(det.input_limits.max_pixels, Some(4_194_304));
        assert_eq!(
            det.input_limits.overflow_behavior,
            DetOverflowBehavior::Downscale
        );

        assert!(cfg.cls.is_none());
        let rec = cfg.rec.as_ref().unwrap();
        assert_eq!(rec.model_path, root.join("rec.onnx"));
        assert_eq!(rec.dict_path, root.join("dict.txt"));
        assert_eq!(rec.image_shape, [3, 48, 320]);
        assert_eq!(rec.batch_size, 6);
    }

    #[test]
    fn pipeline_uses_det_and_rec_without_classifier_on_cpu() {
        let cfg = build_config(&descriptor()).unwrap();

        assert!(cfg.pipeline.use_det);
        assert!(cfg.pipeline.use_rec);
        assert!(!cfg.pipeline.use_cls);
        assert!(cfg.cls.is_none());
        assert_eq!(cfg.inference.execution_provider, ExecutionProvider::Cpu);
    }

    #[test]
    fn detector_input_limits_prevent_unbounded_amplification() {
        let cfg = build_config(&descriptor()).unwrap();
        let limits = cfg.det.as_ref().unwrap().input_limits;

        assert_eq!(limits.max_side_len, Some(4096));
        assert_eq!(limits.max_pixels, Some(4_194_304));
        assert_eq!(limits.overflow_behavior, DetOverflowBehavior::Downscale);
    }

    #[test]
    fn normalize_collapses_unicode_whitespace_and_trims() {
        assert_eq!(normalize_line("  foo\tbar\nbaz  "), "foo bar baz");
        assert_eq!(normalize_line("\u{00A0}a\u{2003}b\u{000B}"), "a b");
        assert_eq!(normalize_line("   "), "");
        assert_eq!(normalize_line(""), "");
        assert_eq!(normalize_line("already"), "already");
    }

    #[test]
    fn output_joins_lines_in_order_and_drops_empty() {
        let timed = TimedOcrOutput {
            output: OcrOutput {
                lines: vec![
                    RapidOcrLine {
                        bbox: Quad::from_xyxy(0.0, 0.0, 10.0, 10.0),
                        text: "  hello   world ".to_string(),
                        score: 0.9,
                    },
                    RapidOcrLine {
                        bbox: Quad::from_xyxy(0.0, 11.0, 10.0, 20.0),
                        text: "   ".to_string(),
                        score: 0.9,
                    },
                    RapidOcrLine {
                        bbox: Quad::from_xyxy(0.0, 21.0, 10.0, 30.0),
                        text: "\tnext\nline\t".to_string(),
                        score: 0.8,
                    },
                ],
            },
            timings: OcrTimings::default(),
        };

        let result = map_output(timed);

        assert_eq!(result.text, "hello world\nnext line");
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "hello world");
        assert_eq!(result.lines[1].text, "next line");
    }

    #[test]
    fn output_mapping_preserves_boxes_scores_and_timing() {
        let points = [[1.0, 2.0], [3.0, 2.0], [3.0, 4.0], [1.0, 4.0]];
        let timed = TimedOcrOutput {
            output: OcrOutput {
                lines: vec![RapidOcrLine {
                    bbox: Quad { points },
                    text: "  a  b ".to_string(),
                    score: 0.77,
                }],
            },
            timings: OcrTimings {
                total_ms: 123.5,
                ..OcrTimings::default()
            },
        };

        let result = map_output(timed);

        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].bbox.points, points);
        assert_eq!(result.lines[0].confidence, 0.77);
        assert_eq!(result.lines[0].text, "a b");
        assert_eq!(result.text, "a b");
        assert_eq!(result.total_ms, 123.5);
    }

    #[test]
    fn every_third_party_worker_error_maps_to_app_category() {
        assert!(matches!(
            map_request_error(TokioOcrError::QueueFull),
            OcrRuntimeError::Busy
        ));
        assert!(matches!(
            map_request_error(TokioOcrError::WorkerStopped),
            OcrRuntimeError::Stopped
        ));
        assert!(matches!(
            map_request_error(TokioOcrError::WorkerPanicked),
            OcrRuntimeError::WorkerPanicked
        ));
        assert!(matches!(
            map_request_error(TokioOcrError::Cancelled),
            OcrRuntimeError::Cancelled
        ));
        assert!(matches!(
            map_request_error(TokioOcrError::TimedOut(Duration::from_secs(15))),
            OcrRuntimeError::Timeout(d) if d == Duration::from_secs(15)
        ));
        assert!(matches!(
            map_request_error(TokioOcrError::Ocr(anyhow::anyhow!("boom"))),
            OcrRuntimeError::Inference
        ));
    }

    #[test]
    fn init_error_retains_safe_model_load_context() {
        match map_init_error(TokioOcrError::Ocr(anyhow::anyhow!(
            "failed to initialize detection stage: model not found"
        ))) {
            OcrRuntimeError::Init(message) => {
                assert!(message.contains("model not found"));
            }
            other => panic!("expected Init, got {other:?}"),
        }

        assert!(matches!(
            map_init_error(TokioOcrError::WorkerPanicked),
            OcrRuntimeError::WorkerPanicked
        ));
        assert!(matches!(
            map_init_error(TokioOcrError::WorkerStopped),
            OcrRuntimeError::Stopped
        ));
    }

    #[test]
    fn init_error_masks_embedded_absolute_paths() {
        match map_init_error(TokioOcrError::Ocr(anyhow::anyhow!(
            r"failed to load model: C:\Users\x\AppData\ttsbard\models\ocr\eslav\det.onnx not found"
        ))) {
            OcrRuntimeError::Init(message) => {
                // Any detected absolute path degrades the whole message to the
                // fixed string; the masked chain is tracing-only.
                assert_eq!(message, INIT_PATH_MASKED_MESSAGE);
                assert!(!message.contains(r"C:\Users\x"));
                assert!(!message.contains("det.onnx"));
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn init_error_without_paths_keeps_the_original_message() {
        match map_init_error(TokioOcrError::Ocr(anyhow::anyhow!(
            "failed to initialize detection stage: unsupported operator"
        ))) {
            OcrRuntimeError::Init(message) => {
                assert_eq!(
                    message,
                    "failed to initialize detection stage: unsupported operator"
                );
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn sanitize_masks_paths_with_spaces_whole() {
        // A username containing a space must not split the mask: the whole
        // path is consumed, so no username fragment or model filename leaks
        // into the traced detail.
        let sanitized = sanitize_init_message(
            r"failed to load model: C:\Users\John Doe\AppData\Roaming\ttsbard\ocr\det.onnx not found",
        );
        assert!(
            !sanitized.contains("John Doe"),
            "username leaked: {sanitized}"
        );
        assert!(!sanitized.contains("John"));
        assert!(!sanitized.contains("Doe"));
        assert!(
            !sanitized.contains("det.onnx"),
            "filename leaked: {sanitized}"
        );
        assert!(sanitized.contains("failed to load model"));
        assert!(sanitized.contains("not found"));
    }

    #[test]
    fn sanitize_masks_each_of_two_adjacent_paths_separately() {
        // A second path after a whitespace is a new mask target, not a
        // continuation of the first.
        let sanitized = sanitize_init_message(r"det C:\a\b.txt C:\c\d.txt missing");
        assert!(!sanitized.contains(r"C:\a"));
        assert!(!sanitized.contains(r"C:\c"));
        assert!(sanitized.contains("det"));
        assert!(sanitized.contains("missing"));
    }
}
