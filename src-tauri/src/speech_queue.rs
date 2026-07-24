use crate::config::{
    AiSettings, AudioEffectsSettings, AudioSettings, DspSettings, NetworkSettings,
};
use crate::preprocessor::TextPreprocessor;
use crate::tts::TtsProvider;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_ACTIVE_CAPACITY: usize = 50;

// ── AcceptedJob: returned by submit ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedJob {
    pub job_id: Uuid,
}

// ── WorkItem: returned by claim_next_generation ──

#[derive(Clone)]
pub(crate) struct WorkItem {
    pub job_id: Uuid,
    pub original_text: String,
    pub snapshot: Snapshot,
}

impl std::fmt::Debug for WorkItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkItem")
            .field("job_id", &self.job_id)
            .field("original_text", &self.original_text)
            .field("snapshot", &"<opaque>")
            .finish()
    }
}

// ── Snapshot: settings frozen at submit time ──

#[derive(Clone)]
pub struct Snapshot {
    pub provider: String,
    pub voice: String,
    pub skip_twitch: bool,
    pub skip_webview: bool,
    pub ai_enabled: bool,
    pub audio_effects: AudioEffectsSettings,
    pub dsp: DspSettings,
    pub audio: AudioSettings,
    pub ai: AiSettings,
    pub tts_provider: TtsProvider,
    pub preprocessor: Option<TextPreprocessor>,
    pub network_settings: NetworkSettings,
}

// ── Job status ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Generating,
    Ready,
    Playing,
    Completed,
    Failed,
    Cancelled,
}

// ── SpeechJob ──

#[derive(Clone)]
pub struct SpeechJob {
    pub job_id: Uuid,
    pub original_text: String,
    pub spoken_text: Option<String>,
    pub status: JobStatus,
    pub error: Option<String>,
    pub attempt: u32,
    pub created_at_ms: i64,
    pub snapshot: Snapshot,
}

impl SpeechJob {
    fn new(original_text: String, snapshot: Snapshot) -> Self {
        Self {
            job_id: Uuid::new_v4(),
            original_text,
            spoken_text: None,
            status: JobStatus::Queued,
            error: None,
            attempt: 1,
            created_at_ms: Utc::now().timestamp_millis(),
            snapshot,
        }
    }
}

// ── Errors ──

#[derive(Debug, Clone, Error)]
pub enum QueueError {
    #[error("empty or whitespace-only text")]
    EmptyText,
    #[error("queue full: maximum {0} active jobs")]
    QueueFull(usize),
    #[error("job not found: {0}")]
    JobNotFound(Uuid),
    #[error("invalid transition: cannot move from {from:?} to {to}")]
    InvalidTransition { from: JobStatus, to: &'static str },
    #[error("job {0} is not the next actionable job")]
    NotNextActionable(Uuid),
}

// ── DTOs ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDto {
    pub job_id: Uuid,
    pub original_text: String,
    pub spoken_text: Option<String>,
    pub status: JobStatus,
    pub error: Option<String>,
    pub attempt: u32,
    pub created_at_ms: i64,
}

impl From<&SpeechJob> for JobDto {
    fn from(job: &SpeechJob) -> Self {
        Self {
            job_id: job.job_id,
            original_text: job.original_text.clone(),
            spoken_text: job.spoken_text.clone(),
            status: job.status,
            error: job.error.clone(),
            attempt: job.attempt,
            created_at_ms: job.created_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechQueueStateDto {
    pub jobs: Vec<JobDto>,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

// ── SpeechQueue ──

pub struct SpeechQueue {
    jobs: VecDeque<SpeechJob>,
}

impl Default for SpeechQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechQueue {
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn submit(&mut self, text: &str, snapshot: Snapshot) -> Result<Uuid, QueueError> {
        if text.trim().is_empty() {
            return Err(QueueError::EmptyText);
        }
        if self.active_count() >= MAX_ACTIVE_CAPACITY {
            return Err(QueueError::QueueFull(MAX_ACTIVE_CAPACITY));
        }
        let job = SpeechJob::new(text.to_string(), snapshot);
        let id = job.job_id;
        self.jobs.push_back(job);
        Ok(id)
    }

    pub fn active_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status != JobStatus::Completed && j.status != JobStatus::Cancelled)
            .count()
    }

    pub fn state(&self) -> SpeechQueueStateDto {
        let blocked = self.jobs.iter().any(|j| j.status == JobStatus::Failed);

        let blocked_reason = if blocked {
            self.jobs
                .iter()
                .find(|j| j.status == JobStatus::Failed)
                .map(|j| {
                    format!(
                        "Job {} failed: {}",
                        j.job_id,
                        j.error.as_deref().unwrap_or("unknown error")
                    )
                })
        } else {
            None
        };

        SpeechQueueStateDto {
            jobs: self.jobs.iter().map(JobDto::from).collect(),
            blocked,
            blocked_reason,
        }
    }

    pub fn next_actionable(&self) -> Option<Uuid> {
        for job in &self.jobs {
            match job.status {
                JobStatus::Failed | JobStatus::Generating => return None,
                JobStatus::Queued => return Some(job.job_id),
                JobStatus::Completed
                | JobStatus::Cancelled
                | JobStatus::Ready
                | JobStatus::Playing => continue,
            }
        }
        None
    }

    fn get_mut(&mut self, job_id: Uuid) -> Result<&mut SpeechJob, QueueError> {
        self.jobs
            .iter_mut()
            .find(|j| j.job_id == job_id)
            .ok_or(QueueError::JobNotFound(job_id))
    }

    pub fn start_generation(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        if !self.jobs.iter().any(|j| j.job_id == job_id) {
            return Err(QueueError::JobNotFound(job_id));
        }
        let actionable = self.next_actionable();
        if actionable != Some(job_id) {
            return Err(QueueError::NotNextActionable(job_id));
        }
        let job = self.get_mut(job_id)?;
        job.status = JobStatus::Generating;
        Ok(())
    }

    pub(crate) fn claim_next_generation(&mut self) -> Result<Option<WorkItem>, QueueError> {
        let job_id = match self.next_actionable() {
            Some(id) => id,
            None => return Ok(None),
        };
        let job = self.get_mut(job_id)?;
        job.status = JobStatus::Generating;
        Ok(Some(WorkItem {
            job_id: job.job_id,
            original_text: job.original_text.clone(),
            snapshot: job.snapshot.clone(),
        }))
    }

    pub fn fail_playback(&mut self, job_id: Uuid, error_msg: String) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        match job.status {
            JobStatus::Ready | JobStatus::Playing => {
                job.status = JobStatus::Failed;
                job.error = Some(error_msg);
                Ok(())
            }
            _ => Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Failed",
            }),
        }
    }

    pub fn mark_ready(&mut self, job_id: Uuid, spoken_text: String) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Generating {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Ready",
            });
        }
        job.status = JobStatus::Ready;
        job.spoken_text = Some(spoken_text);
        Ok(())
    }

    pub fn set_spoken_text(&mut self, job_id: Uuid, text: String) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Generating {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "spoken text",
            });
        }
        job.spoken_text = Some(text);
        Ok(())
    }

    pub fn fail_generation(&mut self, job_id: Uuid, error_msg: String) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Generating {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Failed",
            });
        }
        job.status = JobStatus::Failed;
        job.error = Some(error_msg);
        Ok(())
    }

    pub fn mark_playing(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Ready {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Playing",
            });
        }
        job.status = JobStatus::Playing;
        Ok(())
    }

    pub fn mark_completed(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Playing {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Completed",
            });
        }
        job.status = JobStatus::Completed;
        Ok(())
    }

    pub fn retry_job(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Failed {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Queued (retry)",
            });
        }
        job.status = JobStatus::Queued;
        job.error = None;
        job.spoken_text = None;
        job.attempt += 1;
        Ok(())
    }

    pub fn cancel_job(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        match job.status {
            JobStatus::Queued | JobStatus::Failed => {
                job.status = JobStatus::Cancelled;
                Ok(())
            }
            _ => Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Cancelled",
            }),
        }
    }

    pub fn skip_job(&mut self, job_id: Uuid) -> Result<(), QueueError> {
        let job = self.get_mut(job_id)?;
        if job.status != JobStatus::Failed {
            return Err(QueueError::InvalidTransition {
                from: job.status,
                to: "Skipped (Cancelled)",
            });
        }
        job.status = JobStatus::Cancelled;
        Ok(())
    }

    pub fn has_job(&self, job_id: Uuid) -> bool {
        self.jobs.iter().any(|j| j.job_id == job_id)
    }

    pub fn get_status(&self, job_id: Uuid) -> Option<JobStatus> {
        self.jobs
            .iter()
            .find(|j| j.job_id == job_id)
            .map(|j| j.status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiOpenAiSettings;

    fn snap() -> Snapshot {
        Snapshot {
            provider: "test-provider".into(),
            voice: "test-voice".into(),
            skip_twitch: false,
            skip_webview: false,
            ai_enabled: false,
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            audio: AudioSettings::default(),
            ai: AiSettings::default(),
            tts_provider: TtsProvider::Local(
                crate::tts::local_http_server::LocalHttpServerTts::new(),
            ),
            preprocessor: None,
            network_settings: NetworkSettings::default(),
        }
    }

    fn text_err(e: &QueueError) -> String {
        e.to_string()
    }

    // ── submit ──

    #[test]
    fn submit_creates_job_with_uuid() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello world", snap()).unwrap();
        let state = q.state();
        assert_eq!(state.jobs.len(), 1);
        assert_eq!(state.jobs[0].job_id, id);
        assert_eq!(state.jobs[0].original_text, "hello world");
        assert_eq!(state.jobs[0].status, JobStatus::Queued);
        assert_eq!(state.jobs[0].attempt, 1);
    }

    #[test]
    fn submit_empty_text_rejected() {
        let mut q = SpeechQueue::new();
        let err = q.submit("", snap()).unwrap_err();
        assert!(text_err(&err).contains("empty"));
        assert!(q.is_empty());
    }

    #[test]
    fn submit_whitespace_only_rejected() {
        let mut q = SpeechQueue::new();
        let err = q.submit("   \t\n   ", snap()).unwrap_err();
        assert!(text_err(&err).contains("empty"));
        assert!(q.is_empty());
    }

    #[test]
    fn submit_preserves_original_text_with_whitespace() {
        let mut q = SpeechQueue::new();
        let id = q.submit("  hello world  ", snap()).unwrap();
        let state = q.state();
        assert_eq!(state.jobs[0].original_text, "  hello world  ");
        assert_eq!(state.jobs[0].job_id, id);
    }

    #[test]
    fn submit_preserves_leading_whitespace_text() {
        let mut q = SpeechQueue::new();
        let text = " !hello";
        let id = q.submit(text, snap()).unwrap();
        let state = q.state();
        assert_eq!(state.jobs[0].original_text, " !hello");
        assert_eq!(state.jobs[0].job_id, id);
    }

    #[test]
    fn duplicate_texts_get_distinct_ids() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("hello", snap()).unwrap();
        let id2 = q.submit("hello", snap()).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn submission_fifo_order() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        let id3 = q.submit("third", snap()).unwrap();

        let state = q.state();
        assert_eq!(state.jobs.len(), 3);
        assert_eq!(state.jobs[0].job_id, id1);
        assert_eq!(state.jobs[1].job_id, id2);
        assert_eq!(state.jobs[2].job_id, id3);
    }

    // ── next_actionable ──

    #[test]
    fn next_actionable_first_queued() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();
        assert_eq!(q.next_actionable(), Some(id1));
    }

    #[test]
    fn next_actionable_skips_terminal_states() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        let _id3 = q.submit("third", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.mark_ready(id1, "first".into()).unwrap();
        q.mark_playing(id1).unwrap();
        q.mark_completed(id1).unwrap();

        assert_eq!(q.next_actionable(), Some(id2));
    }

    #[test]
    fn next_actionable_none_when_blocked_by_failure() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "provider error".into()).unwrap();

        assert_eq!(q.next_actionable(), None);
    }

    #[test]
    fn next_actionable_none_when_blocked_by_generating() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();

        assert_eq!(q.next_actionable(), None);
    }

    #[test]
    fn next_actionable_skips_ready_playing_completed_cancelled() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("job1", snap()).unwrap();
        q.start_generation(id1).unwrap();
        q.mark_ready(id1, "text".into()).unwrap();
        q.mark_playing(id1).unwrap();
        q.mark_completed(id1).unwrap();

        let id2 = q.submit("job2", snap()).unwrap();
        q.cancel_job(id2).unwrap();

        let id3 = q.submit("job3", snap()).unwrap();
        assert_eq!(q.next_actionable(), Some(id3));
    }

    // ── claim_next_generation ──

    #[test]
    fn claim_returns_first_queued_with_snapshot() {
        let mut q = SpeechQueue::new();
        let s = snap();
        q.submit("first", s.clone()).unwrap();
        q.submit("second", snap()).unwrap();

        let item = q.claim_next_generation().unwrap().expect("should claim");
        assert_eq!(item.original_text, "first");
        assert_eq!(item.job_id, q.state().jobs[0].job_id);
        assert_eq!(item.snapshot.voice, "test-voice");
    }

    #[test]
    fn claim_returns_none_when_idle() {
        let mut q = SpeechQueue::new();
        assert!(q.claim_next_generation().unwrap().is_none());
    }

    #[test]
    fn claim_prevents_duplicate_in_flight() {
        let mut q = SpeechQueue::new();
        q.submit("first", snap()).unwrap();

        q.claim_next_generation().unwrap().expect("first claim");
        assert!(q.claim_next_generation().unwrap().is_none());
    }

    #[test]
    fn claim_returns_none_when_blocked_by_failed_head() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();

        assert!(q.claim_next_generation().unwrap().is_none());
    }

    #[test]
    fn claim_returns_none_when_blocked_by_generating() {
        let mut q = SpeechQueue::new();
        q.submit("first", snap()).unwrap();
        q.submit("second", snap()).unwrap();

        q.claim_next_generation().unwrap();
        assert!(q.claim_next_generation().unwrap().is_none());
    }

    #[test]
    fn claim_after_retry_of_failed_head() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();
        q.retry_job(id1).unwrap();

        let item = q.claim_next_generation().unwrap().expect("should claim");
        assert_eq!(item.original_text, "first");
    }

    #[test]
    fn claim_after_skip_of_failed_head_advances() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();
        q.skip_job(id1).unwrap();

        let item = q.claim_next_generation().unwrap().expect("should claim");
        assert_eq!(item.original_text, "second");
    }

    // ── fail_playback ──

    #[test]
    fn fail_playback_from_ready() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "spoken".into()).unwrap();

        q.fail_playback(id, "playback error".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error.as_deref(), Some("playback error"));
        assert_eq!(job.spoken_text.as_deref(), Some("spoken"));
        assert_eq!(job.attempt, 1);
    }

    #[test]
    fn fail_playback_from_playing() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "spoken".into()).unwrap();
        q.mark_playing(id).unwrap();

        q.fail_playback(id, "device lost".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error.as_deref(), Some("device lost"));
        assert_eq!(job.spoken_text.as_deref(), Some("spoken"));
        assert_eq!(job.attempt, 1);
    }

    #[test]
    fn fail_playback_rejects_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.fail_playback(id, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);
    }

    #[test]
    fn fail_playback_rejects_generating() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        let err = q.fail_playback(id, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Generating);
    }

    #[test]
    fn fail_playback_rejects_completed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        q.mark_completed(id).unwrap();
        let err = q.fail_playback(id, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn fail_playback_rejects_cancelled() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.cancel_job(id).unwrap();
        let err = q.fail_playback(id, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn fail_playback_rejects_failed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "prev".into()).unwrap();
        let err = q.fail_playback(id, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn fail_playback_unknown_id() {
        let mut q = SpeechQueue::new();
        let fake = Uuid::new_v4();
        let err = q.fail_playback(fake, "err".into()).unwrap_err();
        assert!(text_err(&err).contains("not found"));
    }

    #[test]
    fn fail_playback_preserves_spoken_text_and_attempt() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "preserve-me".into()).unwrap();
        q.fail_playback(id, "oops".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.spoken_text.as_deref(), Some("preserve-me"));
        assert_eq!(job.attempt, 1);
    }

    // ── start_generation ──

    #[test]
    fn start_generation_on_actionable_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        let state = q.state();
        assert_eq!(state.jobs[0].status, JobStatus::Generating);
    }

    #[test]
    fn start_generation_rejects_non_next_job() {
        let mut q = SpeechQueue::new();
        let _id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        let err = q.start_generation(id2).unwrap_err();
        assert!(text_err(&err).contains("not the next actionable job"));
    }

    #[test]
    fn start_generation_rejects_blocked_queue() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "error".into()).unwrap();
        let err = q.start_generation(id2).unwrap_err();
        assert!(text_err(&err).contains("not the next actionable job"));
    }

    #[test]
    fn start_generation_rejects_unknown_id() {
        let mut q = SpeechQueue::new();
        let fake_id = Uuid::new_v4();
        let err = q.start_generation(fake_id).unwrap_err();
        assert!(text_err(&err).contains("not found"));
    }

    #[test]
    fn start_generation_rejects_when_prior_generating() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        q.start_generation(id1).unwrap();
        let err = q.start_generation(id2).unwrap_err();
        assert!(text_err(&err).contains("not the next actionable job"));
    }

    // ── mark_ready ──

    #[test]
    fn mark_ready_from_generating_records_spoken_text() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "hello world".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Ready);
        assert_eq!(job.spoken_text.as_deref(), Some("hello world"));
    }

    #[test]
    fn mark_ready_rejects_from_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.mark_ready(id, "text".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);
    }

    #[test]
    fn retry_clears_spoken_text_set_before_failure() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.set_spoken_text(id, "partial".into()).unwrap();
        q.fail_generation(id, "err".into()).unwrap();
        q.retry_job(id).unwrap();
        assert_eq!(q.state().jobs[0].spoken_text, None);
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);
    }

    #[test]
    fn retry_then_second_attempt_sets_new_spoken_text() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();

        q.start_generation(id).unwrap();
        q.set_spoken_text(id, "v1".into()).unwrap();
        q.fail_generation(id, "err".into()).unwrap();

        q.retry_job(id).unwrap();
        assert_eq!(q.state().jobs[0].spoken_text, None);
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);

        q.start_generation(id).unwrap();
        q.set_spoken_text(id, "v2".into()).unwrap();
        q.mark_ready(id, "v2".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Ready);
        assert_eq!(job.spoken_text.as_deref(), Some("v2"));
    }

    // ── fail_generation ──

    #[test]
    fn fail_generation_sets_failed_and_error() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "TTS timeout".into()).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error.as_deref(), Some("TTS timeout"));
    }

    #[test]
    fn fail_generation_blocks_queue() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "provider down".into()).unwrap();

        let state = q.state();
        assert!(state.blocked);
        assert!(state.blocked_reason.unwrap().contains("provider down"));
    }

    #[test]
    fn fail_generation_preserves_idempotent_state_on_error() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.fail_generation(id, "oops".into()).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);
        assert_eq!(q.state().jobs[0].error, None);
    }

    // ── mark_playing / mark_completed ──

    #[test]
    fn full_happy_path() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();

        q.start_generation(id).unwrap();
        assert_eq!(q.state().jobs[0].status, JobStatus::Generating);

        q.mark_ready(id, "hello".into()).unwrap();
        assert_eq!(q.state().jobs[0].status, JobStatus::Ready);

        q.mark_playing(id).unwrap();
        assert_eq!(q.state().jobs[0].status, JobStatus::Playing);

        q.mark_completed(id).unwrap();
        assert_eq!(q.state().jobs[0].status, JobStatus::Completed);
    }

    #[test]
    fn mark_playing_rejects_from_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.mark_playing(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn mark_completed_rejects_from_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.mark_completed(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn mark_playing_rejects_from_playing() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        let err = q.mark_playing(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn mark_completed_rejects_from_completed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        q.mark_completed(id).unwrap();
        let err = q.mark_completed(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    // ── retry ──

    #[test]
    fn retry_failed_job_resets_to_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "timeout".into()).unwrap();

        q.retry_job(id).unwrap();
        let job = &q.state().jobs[0];
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.error, None);
        assert_eq!(job.spoken_text, None);
        assert_eq!(job.attempt, 2);
    }

    #[test]
    fn retry_preserves_fifo_position_and_unblocks() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();

        assert!(q.state().blocked);

        q.retry_job(id1).unwrap();

        let state = q.state();
        assert!(!state.blocked);
        assert_eq!(state.jobs[0].job_id, id1);
        assert_eq!(state.jobs[1].job_id, id2);
        assert_eq!(q.next_actionable(), Some(id1));
    }

    #[test]
    fn retry_increments_attempt_multiple_times() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "err1".into()).unwrap();

        q.retry_job(id).unwrap();
        assert_eq!(q.state().jobs[0].attempt, 2);

        q.start_generation(id).unwrap();
        q.fail_generation(id, "err2".into()).unwrap();

        q.retry_job(id).unwrap();
        assert_eq!(q.state().jobs[0].attempt, 3);
    }

    #[test]
    fn retry_rejects_non_failed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.retry_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn retry_rejects_completed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        q.mark_completed(id).unwrap();
        let err = q.retry_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn retry_rejects_cancelled() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.cancel_job(id).unwrap();
        let err = q.retry_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    // ── cancel ──

    #[test]
    fn cancel_queued_job() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.cancel_job(id).unwrap();
        assert_eq!(q.state().jobs[0].status, JobStatus::Cancelled);
    }

    #[test]
    fn cancel_failed_job_and_unblock() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "error".into()).unwrap();

        assert!(q.state().blocked);

        q.cancel_job(id1).unwrap();
        let state = q.state();
        assert!(!state.blocked);
        assert_eq!(q.next_actionable(), Some(id2));
    }

    #[test]
    fn cancel_later_job_does_not_unblock_prior_failure() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();
        let id3 = q.submit("third", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "error".into()).unwrap();

        q.cancel_job(id3).unwrap();

        let state = q.state();
        assert!(state.blocked);
        assert_eq!(state.jobs[2].status, JobStatus::Cancelled);
        assert_eq!(state.jobs[1].status, JobStatus::Queued);
    }

    #[test]
    fn cancel_rejects_generating() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        let err = q.cancel_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Generating);
    }

    #[test]
    fn cancel_rejects_completed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        q.mark_completed(id).unwrap();
        let err = q.cancel_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    // ── skip ──

    #[test]
    fn skip_rejects_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Queued);
    }

    #[test]
    fn skip_failed_job_cancels_and_unblocks() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();
        assert!(q.state().blocked);

        q.skip_job(id1).unwrap();
        let state = q.state();
        assert!(!state.blocked);
        assert_eq!(state.jobs[0].status, JobStatus::Cancelled);
        assert_eq!(q.next_actionable(), Some(id2));
    }

    #[test]
    fn skip_rejects_generating() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Generating);
    }

    #[test]
    fn skip_rejects_completed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        q.mark_completed(id).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
    }

    #[test]
    fn skip_rejects_ready() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Ready);
    }

    #[test]
    fn skip_rejects_playing() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.mark_ready(id, "text".into()).unwrap();
        q.mark_playing(id).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Playing);
    }

    #[test]
    fn skip_rejects_cancelled() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.cancel_job(id).unwrap();
        let err = q.skip_job(id).unwrap_err();
        assert!(text_err(&err).contains("invalid transition"));
        assert_eq!(q.state().jobs[0].status, JobStatus::Cancelled);
    }

    #[test]
    fn skip_unknown_id_errors() {
        let mut q = SpeechQueue::new();
        let fake = Uuid::new_v4();
        let err = q.skip_job(fake).unwrap_err();
        assert!(text_err(&err).contains("not found"));
    }

    // ── capacity ──

    #[test]
    fn capacity_accepts_up_to_max() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            let text = format!("job {}", i);
            let id = q.submit(&text, snap()).unwrap();
            assert!(!id.is_nil());
        }
        assert_eq!(q.active_count(), MAX_ACTIVE_CAPACITY);
    }

    #[test]
    fn capacity_rejects_when_full() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            q.submit(&format!("job {}", i), snap()).unwrap();
        }
        let err = q.submit("overflow", snap()).unwrap_err();
        assert!(text_err(&err).contains("queue full"));
        assert_eq!(q.active_count(), MAX_ACTIVE_CAPACITY);
    }

    #[test]
    fn completed_jobs_do_not_consume_capacity() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            let id = q.submit(&format!("job {}", i), snap()).unwrap();
            q.start_generation(id).unwrap();
            q.mark_ready(id, format!("text {}", i)).unwrap();
            q.mark_playing(id).unwrap();
            q.mark_completed(id).unwrap();
        }
        assert_eq!(q.active_count(), 0);
        let id = q.submit("new after completed", snap()).unwrap();
        assert!(!id.is_nil());
    }

    #[test]
    fn cancelled_jobs_do_not_consume_capacity() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            let id = q.submit(&format!("job {}", i), snap()).unwrap();
            q.cancel_job(id).unwrap();
        }
        assert_eq!(q.active_count(), 0);
        let id = q.submit("new after cancelled", snap()).unwrap();
        assert!(!id.is_nil());
    }

    #[test]
    fn mixed_active_and_terminal_capacity() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            let id = q.submit(&format!("completed {}", i), snap()).unwrap();
            q.start_generation(id).unwrap();
            q.mark_ready(id, format!("text {}", i)).unwrap();
            q.mark_playing(id).unwrap();
            q.mark_completed(id).unwrap();
        }
        for i in 0..MAX_ACTIVE_CAPACITY {
            q.submit(&format!("queued {}", i), snap()).unwrap();
        }
        assert_eq!(q.active_count(), MAX_ACTIVE_CAPACITY);
        let err = q.submit("overflow", snap()).unwrap_err();
        assert!(text_err(&err).contains("queue full"));
    }

    #[test]
    fn capacity_queue_full_preserves_order() {
        let mut q = SpeechQueue::new();
        for i in 0..MAX_ACTIVE_CAPACITY {
            q.submit(&format!("job {}", i), snap()).unwrap();
        }
        let before = q.state();
        let _ = q.submit("should be rejected", snap());
        let after = q.state();
        assert_eq!(before.jobs.len(), after.jobs.len());
        for (a, b) in before.jobs.iter().zip(after.jobs.iter()) {
            assert_eq!(a.job_id, b.job_id);
            assert_eq!(a.status, b.status);
        }
    }

    // ── unknown id ──

    #[test]
    fn unknown_id_errors_on_all_operations() {
        let mut q = SpeechQueue::new();
        let fake = Uuid::new_v4();

        for op_name in &[
            "start_generation",
            "set_spoken_text",
            "mark_ready",
            "fail_generation",
            "mark_playing",
            "mark_completed",
            "retry_job",
            "cancel_job",
            "skip_job",
        ] {
            let err = match *op_name {
                "start_generation" => q.start_generation(fake).unwrap_err(),
                "set_spoken_text" => q.set_spoken_text(fake, "err".into()).unwrap_err(),
                "mark_ready" => q.mark_ready(fake, "err".into()).unwrap_err(),
                "fail_generation" => q.fail_generation(fake, "err".into()).unwrap_err(),
                "mark_playing" => q.mark_playing(fake).unwrap_err(),
                "mark_completed" => q.mark_completed(fake).unwrap_err(),
                "retry_job" => q.retry_job(fake).unwrap_err(),
                "cancel_job" => q.cancel_job(fake).unwrap_err(),
                "skip_job" => q.skip_job(fake).unwrap_err(),
                _ => unreachable!(),
            };
            assert!(
                text_err(&err).contains("not found"),
                "{op_name}: expected 'not found', got: {err}"
            );
        }
    }

    // ── state DTO ──

    #[test]
    fn state_dto_reflects_fifo_order() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();
        let id3 = q.submit("third", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.mark_ready(id1, "first".into()).unwrap();
        q.start_generation(id2).unwrap();
        q.fail_generation(id2, "err".into()).unwrap();

        let state = q.state();
        assert_eq!(state.jobs.len(), 3);
        assert_eq!(state.jobs[0].job_id, id1);
        assert_eq!(state.jobs[0].status, JobStatus::Ready);
        assert_eq!(state.jobs[1].job_id, id2);
        assert_eq!(state.jobs[1].status, JobStatus::Failed);
        assert_eq!(state.jobs[2].job_id, id3);
        assert_eq!(state.jobs[2].status, JobStatus::Queued);
        assert!(state.blocked);
    }

    #[test]
    fn state_dto_not_blocked_after_all_completed() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let id2 = q.submit("second", snap()).unwrap();

        q.start_generation(id1).unwrap();
        q.mark_ready(id1, "first".into()).unwrap();
        q.mark_playing(id1).unwrap();
        q.mark_completed(id1).unwrap();

        q.start_generation(id2).unwrap();
        q.mark_ready(id2, "second".into()).unwrap();
        q.mark_playing(id2).unwrap();
        q.mark_completed(id2).unwrap();

        let state = q.state();
        assert!(!state.blocked);
        assert!(state.blocked_reason.is_none());
    }

    #[test]
    fn state_dto_not_blocked_when_empty() {
        let q = SpeechQueue::new();
        let state = q.state();
        assert!(!state.blocked);
        assert!(state.jobs.is_empty());
    }

    #[test]
    fn state_dto_blocked_when_only_failed_with_no_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("only", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "err".into()).unwrap();
        let state = q.state();
        assert!(state.blocked);
        assert!(state.blocked_reason.is_some());
    }

    // ── idempotent errors ──

    #[test]
    fn errors_do_not_mutate_state() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        let before = serde_json::to_value(q.state()).unwrap();

        let _ = q.mark_ready(id, "text".into()); // expect error: not generating

        let after = serde_json::to_value(q.state()).unwrap();
        assert_eq!(before, after);
    }

    // ── timestamp type ──

    #[test]
    fn created_at_ms_is_i64_timestamp() {
        let before_ms = Utc::now().timestamp_millis();
        let mut q = SpeechQueue::new();
        q.submit("hello", snap()).unwrap();
        let after_ms = Utc::now().timestamp_millis();
        let ts = q.state().jobs[0].created_at_ms;
        assert!(ts >= before_ms);
        assert!(ts <= after_ms);
    }

    // ── blocked invariants ──

    #[test]
    fn queue_blocked_when_failed_with_subsequent_queued() {
        let mut q = SpeechQueue::new();
        let id1 = q.submit("first", snap()).unwrap();
        let _id2 = q.submit("second", snap()).unwrap();
        q.start_generation(id1).unwrap();
        q.fail_generation(id1, "err".into()).unwrap();
        assert!(q.state().blocked);
    }

    #[test]
    fn queue_blocked_when_failed_without_subsequent_queued() {
        let mut q = SpeechQueue::new();
        let id = q.submit("only", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "err".into()).unwrap();
        assert!(q.state().blocked);
    }

    #[test]
    fn queue_unblocked_after_retry_of_only_failed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("only", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "err".into()).unwrap();
        assert!(q.state().blocked);
        q.retry_job(id).unwrap();
        assert!(!q.state().blocked);
    }

    #[test]
    fn queue_unblocked_after_cancel_of_only_failed() {
        let mut q = SpeechQueue::new();
        let id = q.submit("only", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "err".into()).unwrap();
        assert!(q.state().blocked);
        q.cancel_job(id).unwrap();
        assert!(!q.state().blocked);
    }

    #[test]
    fn blocked_reason_preserved_after_retry_clears() {
        let mut q = SpeechQueue::new();
        let id = q.submit("test", snap()).unwrap();
        q.start_generation(id).unwrap();
        q.fail_generation(id, "disk full".into()).unwrap();
        let reason = q.state().blocked_reason.unwrap();
        assert!(reason.contains("disk full"));
        q.retry_job(id).unwrap();
        assert!(q.state().blocked_reason.is_none());
    }

    // ── serialization regression ──

    #[test]
    fn serialization_regression() {
        // WorkItem deliberately has no Serialize/Deserialize derive.
        // Verify that state DTO JSON never contains snapshot or work_item.
        let mut q = SpeechQueue::new();
        q.submit("hello", snap()).unwrap();
        let json = serde_json::to_string(&q.state()).unwrap();
        assert!(
            !json.contains("snapshot"),
            "state JSON must not contain 'snapshot'"
        );
        assert!(
            !json.contains("work_item"),
            "state JSON must not contain 'work_item'"
        );
        assert!(
            !json.contains("WorkItem"),
            "state JSON must not contain 'WorkItem'"
        );
    }

    // ── has_job / get_status ──

    #[test]
    fn has_job_returns_true_for_existing() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        assert!(q.has_job(id));
    }

    #[test]
    fn has_job_returns_false_for_unknown() {
        let q = SpeechQueue::new();
        assert!(!q.has_job(Uuid::new_v4()));
    }

    #[test]
    fn get_status_returns_correct_status() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        assert_eq!(q.get_status(id), Some(JobStatus::Queued));
    }

    #[test]
    fn get_status_returns_none_for_unknown() {
        let q = SpeechQueue::new();
        assert_eq!(q.get_status(Uuid::new_v4()), None);
    }

    #[test]
    fn get_status_tracks_transitions() {
        let mut q = SpeechQueue::new();
        let id = q.submit("hello", snap()).unwrap();
        q.start_generation(id).unwrap();
        assert_eq!(q.get_status(id), Some(JobStatus::Generating));
        q.mark_ready(id, "text".into()).unwrap();
        assert_eq!(q.get_status(id), Some(JobStatus::Ready));
        q.mark_playing(id).unwrap();
        assert_eq!(q.get_status(id), Some(JobStatus::Playing));
        q.mark_completed(id).unwrap();
        assert_eq!(q.get_status(id), Some(JobStatus::Completed));
    }

    #[test]
    fn state_json_excludes_snapshot_and_api_key() {
        let snapshot = Snapshot {
            provider: "test".into(),
            voice: "alloy".into(),
            skip_twitch: false,
            skip_webview: false,
            ai_enabled: true,
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            audio: AudioSettings::default(),
            ai: AiSettings {
                openai: AiOpenAiSettings {
                    api_key: Some("sk-test-sentinel-dont-expose".into()),
                    ..Default::default()
                },
                ..Default::default()
            },
            tts_provider: TtsProvider::Local(
                crate::tts::local_http_server::LocalHttpServerTts::new(),
            ),
            preprocessor: None,
            network_settings: NetworkSettings::default(),
        };
        let mut q = SpeechQueue::new();
        q.submit("hello", snapshot).unwrap();
        let json = serde_json::to_string(&q.state()).unwrap();
        assert!(
            !json.contains("snapshot"),
            "state JSON must not contain snapshot field"
        );
        assert!(
            !json.contains("sk-test-sentinel-dont-expose"),
            "state JSON must not contain AI API key"
        );
    }
}
