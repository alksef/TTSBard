use crate::audio::{open_sink_on_device_pcm, resolve_output_device, AudioPcm, OutputConfig};
use chrono::Utc;
use parking_lot::RwLock;
use rodio::{OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, warn};

const AUDIO_CACHE_SIZE: usize = 20;
const MAX_QUEUE: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybackStatus {
    Idle,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct QueuedPhrase {
    pub id: String,
    pub text: String,
    pub audio: Arc<AudioPcm>,
    pub speaker: Option<OutputConfig>,
    pub mic: Option<OutputConfig>,
}

#[derive(Clone)]
struct CachedPhrase {
    id: String,
    text: String,
    audio: Arc<AudioPcm>,
    timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPhrase {
    pub id: String,
    pub text: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStateDto {
    pub status: PlaybackStatus,
    pub current: Option<String>,
    pub current_id: Option<String>,
    pub queue: Vec<String>,
    pub recent: Vec<RecentPhrase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackActivityDto {
    pub rows: Vec<ActivityRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRow {
    pub id: String,
    pub job_id: Option<String>,
    pub original_text: String,
    pub spoken_text: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub attempt: u32,
    pub created_at_ms: i64,
    pub last_activity_at_ms: i64,
    pub is_current: bool,
    pub can_replay: bool,
}

fn job_status_activity_str(
    status: &crate::speech_queue::JobStatus,
    is_paused: bool,
) -> &'static str {
    use crate::speech_queue::JobStatus;
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Generating => "generating",
        JobStatus::Ready => "ready",
        JobStatus::Playing => {
            if is_paused {
                "paused"
            } else {
                "playing"
            }
        }
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn playback_status_str(pb_status: &PlaybackStatus) -> &'static str {
    match pb_status {
        PlaybackStatus::Idle => "idle",
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
        PlaybackStatus::Stopped => "stopped",
    }
}

pub fn project_playback_activity(
    jobs: &[crate::speech_queue::JobDto],
    cache_entries: &[(String, String, i64)],
    current_id: &Option<String>,
    queue_ids: &[String],
    pb_status: &PlaybackStatus,
) -> Vec<ActivityRow> {
    use crate::speech_queue::JobStatus;
    use std::collections::HashSet;

    let is_paused = *pb_status == PlaybackStatus::Paused;

    let replay_current_or_queued: HashSet<&String> = {
        let mut s = HashSet::new();
        if let Some(cid) = current_id {
            s.insert(cid);
        }
        for qid in queue_ids {
            s.insert(qid);
        }
        s
    };

    let mut rows: Vec<ActivityRow> = Vec::new();
    let mut matched_cache: HashSet<String> = HashSet::new();

    for job in jobs {
        let job_id_str = job.job_id.to_string();
        let in_cache = cache_entries.iter().any(|(id, _, _)| id == &job_id_str);
        matched_cache.insert(job_id_str.clone());

        let is_current = current_id.as_deref() == Some(&job_id_str);
        let is_current_or_queued = replay_current_or_queued.contains(&job_id_str);

        let status = if is_current && *pb_status == PlaybackStatus::Stopped {
            "stopped"
        } else if job.status == JobStatus::Ready {
            if is_current {
                if is_paused {
                    "paused"
                } else {
                    "playing"
                }
            } else if is_current_or_queued {
                "replay_queued"
            } else {
                "ready"
            }
        } else if job.status == JobStatus::Completed {
            if is_current {
                if is_paused {
                    "paused"
                } else {
                    "playing"
                }
            } else if is_current_or_queued {
                "replay_queued"
            } else {
                "completed"
            }
        } else if job.status == JobStatus::Cancelled
            && job.error.is_none()
            && job.spoken_text.is_some()
        {
            if is_current {
                if is_paused {
                    "paused"
                } else {
                    "playing"
                }
            } else if is_current_or_queued {
                "replay_queued"
            } else {
                "cancelled"
            }
        } else {
            job_status_activity_str(&job.status, is_paused && is_current)
        };

        let is_in_queue = queue_ids.iter().any(|q| q == &job_id_str);
        let can_replay = in_cache
            && !is_in_queue
            && (job.status == JobStatus::Completed || *pb_status == PlaybackStatus::Stopped)
            && !(is_current && *pb_status != PlaybackStatus::Stopped);

        rows.push(ActivityRow {
            id: job_id_str.clone(),
            job_id: Some(job_id_str.clone()),
            original_text: job.original_text.clone(),
            spoken_text: job.spoken_text.clone(),
            status: status.to_string(),
            error: job.error.clone(),
            attempt: job.attempt,
            created_at_ms: job.created_at_ms,
            last_activity_at_ms: job.last_activity_at_ms,
            is_current,
            can_replay,
        });
    }

    for (cache_id, cache_text, cache_ts) in cache_entries {
        if matched_cache.contains(cache_id) {
            continue;
        }
        let is_current = current_id.as_deref() == Some(cache_id.as_str());
        let is_in_queue = queue_ids.iter().any(|q| q == cache_id);

        let status = if is_current {
            playback_status_str(pb_status).to_string()
        } else if is_in_queue {
            "replay_queued".to_string()
        } else {
            "completed".to_string()
        };

        let can_replay =
            (!is_current && !is_in_queue) || (is_current && *pb_status == PlaybackStatus::Stopped);

        rows.push(ActivityRow {
            id: cache_id.clone(),
            job_id: None,
            original_text: cache_text.clone(),
            spoken_text: None,
            status,
            error: None,
            attempt: 1,
            created_at_ms: *cache_ts,
            last_activity_at_ms: *cache_ts,
            is_current,
            can_replay,
        });
    }

    rows.sort_by(|a, b| {
        b.last_activity_at_ms
            .cmp(&a.last_activity_at_ms)
            .then_with(|| a.id.cmp(&b.id))
    });

    rows
}

/// Динамическая конфигурация аудиовыходов — обновляется в runtime.
/// Хранится в `Arc<RwLock<>>` и читается потоком на каждый Enqueue.
#[derive(Clone)]
pub struct AudioOutputsConfig {
    pub speaker: Option<OutputConfig>,
    pub mic: Option<OutputConfig>,
}

enum Cmd {
    Enqueue(QueuedPhrase),
    Pause,
    Resume,
    Stop,
    Repeat,
}

struct Shared {
    status: PlaybackStatus,
    current: Option<QueuedPhrase>,
    queue: VecDeque<QueuedPhrase>,
    audio_cache: VecDeque<CachedPhrase>,
}

#[derive(Debug)]
enum EnqueueState {
    SendToThread(QueuedPhrase),
    Queued,
    Rejected,
}

impl Shared {
    fn enqueue_state(
        &mut self,
        id: String,
        text: String,
        audio: Arc<AudioPcm>,
        speaker: Option<OutputConfig>,
        mic: Option<OutputConfig>,
    ) -> EnqueueState {
        let ts = Utc::now().timestamp_millis();
        self.audio_cache.retain(|c| c.id != id);
        self.audio_cache.push_back(CachedPhrase {
            id: id.clone(),
            text: text.clone(),
            audio: Arc::clone(&audio),
            timestamp: ts,
        });
        if self.audio_cache.len() > AUDIO_CACHE_SIZE {
            self.audio_cache.pop_front();
        }

        if self.current.is_some()
            && (self.status == PlaybackStatus::Playing || self.status == PlaybackStatus::Paused)
        {
            let already_current = self.current.as_ref().map(|c| c.id == id).unwrap_or(false);
            if already_current {
                return EnqueueState::Queued;
            }
            let already_queued = self.queue.iter().any(|q| q.id == id);
            if already_queued {
                return EnqueueState::Queued;
            }
            if self.queue.len() < MAX_QUEUE {
                self.queue.push_back(QueuedPhrase {
                    id,
                    text,
                    audio,
                    speaker,
                    mic,
                });
                return EnqueueState::Queued;
            }
            return EnqueueState::Rejected;
        }

        let phrase = QueuedPhrase {
            id: id.clone(),
            text,
            audio,
            speaker,
            mic,
        };
        self.current = Some(phrase.clone());
        EnqueueState::SendToThread(phrase)
    }

    fn can_pause(&self) -> bool {
        self.current.is_some()
    }

    fn can_resume(&self) -> bool {
        self.current.is_some() && self.status == PlaybackStatus::Paused
    }

    fn can_stop(&self) -> bool {
        self.current.is_some()
    }

    fn can_repeat(&self) -> bool {
        self.current.is_some()
    }

    fn finish_inner(&mut self) -> Option<QueuedPhrase> {
        if let Some(next) = self.queue.pop_front() {
            self.current = Some(next.clone());
            Some(next)
        } else {
            self.current = None;
            self.status = PlaybackStatus::Idle;
            None
        }
    }

    fn get_state_dto(&self) -> PlaybackStateDto {
        PlaybackStateDto {
            status: self.status.clone(),
            current: self.current.as_ref().map(|p| p.text.clone()),
            current_id: self.current.as_ref().map(|p| p.id.clone()),
            queue: self.queue.iter().map(|p| p.text.clone()).collect(),
            recent: self
                .audio_cache
                .iter()
                .rev()
                .take(5)
                .map(|c| RecentPhrase {
                    id: c.id.clone(),
                    text: c.text.clone(),
                    timestamp: c.timestamp,
                })
                .collect(),
        }
    }

    fn find_in_cache(&self, id: &str) -> Option<(String, String, Arc<AudioPcm>)> {
        self.audio_cache
            .iter()
            .find(|c| c.id == id)
            .map(|c| (c.id.clone(), c.text.clone(), Arc::clone(&c.audio)))
    }

    fn all_cache_entries(&self) -> Vec<(String, String, i64)> {
        self.audio_cache
            .iter()
            .map(|c| (c.id.clone(), c.text.clone(), c.timestamp))
            .collect()
    }

    fn has_cached_audio(&self, id: &str) -> bool {
        self.audio_cache.iter().any(|c| c.id == id)
    }

    fn current_identity(&self) -> Option<(String, String)> {
        self.current
            .as_ref()
            .map(|p| (p.id.clone(), p.text.clone()))
    }

    fn queued_ids(&self) -> Vec<String> {
        self.queue.iter().map(|q| q.id.clone()).collect()
    }

    fn remove_queued_item(&mut self, id: &str) -> Result<(), String> {
        let pos = self.queue.iter().position(|q| q.id == id);
        match pos {
            Some(idx) => {
                self.queue.remove(idx);
                Ok(())
            }
            None => {
                if self.current.as_ref().map(|c| c.id == id).unwrap_or(false) {
                    Err(format!(
                        "NotQueued: id '{}' is already current, not in queue",
                        id
                    ))
                } else {
                    Err(format!(
                        "NotFound: id '{}' is not in the playback queue",
                        id
                    ))
                }
            }
        }
    }

    fn accept_replay(
        &mut self,
        id: &str,
        text: String,
        audio: Arc<AudioPcm>,
        speaker: Option<OutputConfig>,
        mic: Option<OutputConfig>,
    ) -> Result<EnqueueState, String> {
        let is_queued = self.queue.iter().any(|q| q.id == id);
        if is_queued {
            return Err(format!(
                "AlreadyPending: id '{}' is already queued for playback",
                id
            ));
        }

        let is_active = self.current.is_some()
            && (self.status == PlaybackStatus::Playing || self.status == PlaybackStatus::Paused);

        if is_active {
            let is_current = self.current.as_ref().map(|c| c.id == id).unwrap_or(false);
            if is_current {
                return Err(format!(
                    "AlreadyPending: id '{}' is already current for playback",
                    id
                ));
            }
            if self.queue.len() >= MAX_QUEUE {
                return Err(format!(
                    "QueueFull: playback queue is full (max {})",
                    MAX_QUEUE
                ));
            }
        }

        let now = Utc::now().timestamp_millis();
        if let Some(entry) = self.audio_cache.iter_mut().find(|c| c.id == id) {
            if now > entry.timestamp {
                entry.timestamp = now;
            } else {
                entry.timestamp = entry.timestamp.saturating_add(1);
            }
        }

        if is_active {
            self.queue.push_back(QueuedPhrase {
                id: id.to_string(),
                text,
                audio,
                speaker,
                mic,
            });
            Ok(EnqueueState::Queued)
        } else {
            let phrase = QueuedPhrase {
                id: id.to_string(),
                text,
                audio,
                speaker,
                mic,
            };
            self.current = Some(phrase.clone());
            Ok(EnqueueState::SendToThread(phrase))
        }
    }
}

pub struct PlaybackSnapshot {
    pub cache_entries: Vec<(String, String, i64)>,
    pub current_id: Option<String>,
    pub queue_ids: Vec<String>,
    pub pb_status: PlaybackStatus,
}

pub struct PlaybackManager {
    cmd_tx: mpsc::Sender<Cmd>,
    state: Arc<RwLock<Shared>>,
    pub audio_config: Arc<RwLock<AudioOutputsConfig>>,
    app_handle: AppHandle,
}

impl PlaybackManager {
    pub fn new(
        app_handle: AppHandle,
        internal_ev: mpsc::Sender<crate::events::AppEvent>,
        initial_audio: AudioOutputsConfig,
        cached_devices: Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let state = Arc::new(RwLock::new(Shared {
            status: PlaybackStatus::Idle,
            current: None,
            queue: VecDeque::new(),
            audio_cache: VecDeque::with_capacity(AUDIO_CACHE_SIZE),
        }));
        let audio_config = Arc::new(RwLock::new(initial_audio));

        let th_state = Arc::clone(&state);
        let th_audio = Arc::clone(&audio_config);
        let th_devices = cached_devices.clone();
        let th_cmd_tx = cmd_tx.clone();
        let th_app = app_handle.clone();

        thread::spawn(move || {
            Self::thread_loop(
                cmd_rx,
                th_cmd_tx,
                th_app,
                internal_ev,
                th_state,
                th_audio,
                th_devices,
            );
        });

        PlaybackManager {
            cmd_tx,
            state,
            audio_config,
            app_handle,
        }
    }

    fn thread_loop(
        cmd_rx: Receiver<Cmd>,
        cmd_tx: mpsc::Sender<Cmd>,
        app: AppHandle,
        internal_ev: mpsc::Sender<crate::events::AppEvent>,
        state: Arc<RwLock<Shared>>,
        _audio_config: Arc<RwLock<AudioOutputsConfig>>, // held; no longer read directly — phrases carry their own outputs
        cached_devices: Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
    ) {
        let mut sink_spk: Option<Sink> = None;
        let mut sink_mic: Option<Sink> = None;
        let mut _stream_spk: Option<OutputStream> = None;
        let mut _stream_mic: Option<OutputStream> = None;
        let mut playing = false;
        let mut stopped = false;

        loop {
            let cmd = if playing && !stopped {
                cmd_rx.recv_timeout(Duration::from_millis(50))
            } else {
                match cmd_rx.recv() {
                    Ok(c) => Ok(c),
                    Err(_) => Err(RecvTimeoutError::Disconnected),
                }
            };

            match cmd {
                Ok(Cmd::Enqueue(phrase)) => {
                    info!(target: "playback", text=%phrase.text, "Enqueue received");
                    if playing {
                        continue;
                    }
                    stopped = false;

                    let spk_cfg = phrase.speaker.clone();
                    let mic_cfg = phrase.mic.clone();
                    let audio = phrase.audio.clone();

                    if let Some(ref c) = spk_cfg {
                        match resolve_output_device(&c.device_id, &cached_devices) {
                            Ok(dev) => match open_sink_on_device_pcm(&dev, &audio, c.volume) {
                                Ok((s, sink)) => {
                                    sink_spk = Some(sink);
                                    _stream_spk = Some(s);
                                }
                                Err(e) => {
                                    warn!(target = "playback", error = %e, "speaker open_sink failed")
                                }
                            },
                            Err(e) => {
                                warn!(target = "playback", error = %e, "speaker device resolve failed")
                            }
                        }
                    }
                    if let Some(ref c) = mic_cfg {
                        match resolve_output_device(&c.device_id, &cached_devices) {
                            Ok(dev) => match open_sink_on_device_pcm(&dev, &audio, c.volume) {
                                Ok((s, sink)) => {
                                    sink_mic = Some(sink);
                                    _stream_mic = Some(s);
                                }
                                Err(e) => {
                                    warn!(target = "playback", error = %e, "mic open_sink failed")
                                }
                            },
                            Err(e) => {
                                warn!(target = "playback", error = %e, "mic device resolve failed")
                            }
                        }
                    }

                    info!(target: "playback", has_spk=sink_spk.is_some(), has_mic=sink_mic.is_some(), "Playback start check");
                    if sink_spk.is_some() || sink_mic.is_some() {
                        playing = true;
                        state.write().status = PlaybackStatus::Playing;
                        let _ = internal_ev.send(crate::events::AppEvent::PlaybackStarted {
                            text_id: phrase.id.clone(),
                            text: phrase.text.clone(),
                        });
                        let _ = app.emit(
                            "playback-started",
                            serde_json::json!({
                                "text_id": phrase.id,
                                "text": phrase.text,
                            }),
                        );
                        info!(target: "playback", "PlaybackStarted emitted");
                    } else {
                        warn!(target: "playback", "No output sink — playback NOT started (speaker+mic both failed)");
                        let failed_id = phrase.id.clone();
                        let _ = internal_ev.send(crate::events::AppEvent::PlaybackFailed {
                            text_id: failed_id.clone(),
                            error: "No output sink could be opened (speaker and mic both failed)"
                                .to_string(),
                        });
                        let _ = app.emit(
                            "playback-failed",
                            serde_json::json!({
                                "text_id": failed_id,
                                "error": "No output sink could be opened",
                            }),
                        );
                    }
                }
                Ok(Cmd::Pause) => {
                    if sink_spk.is_none() && sink_mic.is_none() {
                        continue;
                    }
                    if let Some(ref s) = sink_spk {
                        s.pause();
                    }
                    if let Some(ref s) = sink_mic {
                        s.pause();
                    }
                    state.write().status = PlaybackStatus::Paused;
                    let _ = internal_ev.send(crate::events::AppEvent::PlaybackPaused);
                    let _ = app.emit("playback-paused", ());
                }
                Ok(Cmd::Resume) => {
                    if sink_spk.is_none() && sink_mic.is_none() {
                        continue;
                    }
                    if let Some(ref s) = sink_spk {
                        s.play();
                    }
                    if let Some(ref s) = sink_mic {
                        s.play();
                    }
                    state.write().status = PlaybackStatus::Playing;
                    let _ = internal_ev.send(crate::events::AppEvent::PlaybackResumed);
                    let _ = app.emit("playback-resumed", ());
                }
                Ok(Cmd::Stop) => {
                    sink_spk.take();
                    sink_mic.take();
                    _stream_spk.take();
                    _stream_mic.take();
                    playing = false;
                    stopped = true;
                    state.write().status = PlaybackStatus::Stopped;
                    let _ = internal_ev.send(crate::events::AppEvent::PlaybackStopped);
                    let _ = app.emit("playback-stopped", ());
                }
                Ok(Cmd::Repeat) => {
                    if sink_spk.is_none() && sink_mic.is_none() {
                        warn!("Repeat: nothing playing");
                        continue;
                    }
                    let was_paused = sink_spk.as_ref().map(|s| s.is_paused()).unwrap_or(false)
                        || sink_mic.as_ref().map(|s| s.is_paused()).unwrap_or(false);
                    let seek_ok = sink_spk
                        .as_ref()
                        .map(|s| s.try_seek(Duration::ZERO).is_ok())
                        .unwrap_or(true)
                        && sink_mic
                            .as_ref()
                            .map(|s| s.try_seek(Duration::ZERO).is_ok())
                            .unwrap_or(true);
                    if !seek_ok {
                        // fallback: re-enqueue from cache (M9)
                        let phrase = state.read().current.clone();
                        if let Some(p) = phrase {
                            let _ = cmd_tx.send(Cmd::Stop);
                            let _ = cmd_tx.send(Cmd::Enqueue(p));
                        }
                    } else {
                        if let Some(ref s) = sink_spk {
                            s.play();
                        }
                        if let Some(ref s) = sink_mic {
                            s.play();
                        }
                        if was_paused {
                            state.write().status = PlaybackStatus::Playing;
                            let _ = internal_ev.send(crate::events::AppEvent::PlaybackResumed);
                            let _ = app.emit("playback-resumed", ());
                        }
                        playing = true;
                        stopped = false;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            if playing && !stopped {
                let spk_done = sink_spk.as_ref().map(|s| s.empty()).unwrap_or(true);
                let mic_done = sink_mic.as_ref().map(|s| s.empty()).unwrap_or(true);
                let paused = sink_spk.as_ref().map(|s| s.is_paused()).unwrap_or(false)
                    || sink_mic.as_ref().map(|s| s.is_paused()).unwrap_or(false);

                if !paused && spk_done && mic_done {
                    debug!(target: "playback", "Sinks drained, playing=false");
                    playing = false;
                    sink_spk.take();
                    sink_mic.take();
                    _stream_spk.take();
                    _stream_mic.take();
                    let finished_id = {
                        let mut s = state.write();
                        let id = s.current.as_ref().map(|p| p.id.clone());
                        s.status = PlaybackStatus::Idle;
                        id
                    };
                    if let Some(id) = finished_id {
                        info!(target: "playback", text_id=%id, "PlaybackFinished, playing reset");
                        let _ = internal_ev
                            .send(crate::events::AppEvent::PlaybackFinished { text_id: id });
                    } else {
                        warn!(target: "playback", "Sink drain with no current phrase — invariant violated");
                    }
                    let _ = app.emit("queue-changed", ());
                }
            }
        }

        info!("Playback thread ended");
    }

    /// Обновить динамическую конфигурацию аудиовыходов (C1-дыра).
    /// Вызывается из `speak_text_internal` перед/после `enqueue`.
    pub fn update_audio_config(&self, speaker: Option<OutputConfig>, mic: Option<OutputConfig>) {
        *self.audio_config.write() = AudioOutputsConfig { speaker, mic };
    }

    /// Добавить фразу в очередь. Snapshots текущий глобальный `audio_config` —
    /// compatibility wrapper для legacy вызовов без явного per-phrase output config.
    /// Возвращает `true` если фраза принята, `false` если очередь полна.
    pub fn enqueue(&self, id: String, text: String, audio: AudioPcm) -> bool {
        let cfg = self.audio_config.read().clone();
        self.enqueue_inner(id, text, audio, cfg.speaker, cfg.mic)
    }

    /// Добавить фразу с явным per-phrase output config. Выходные устройства
    /// фиксируются в QueuedPhrase и воспроизводятся потоком без доступа к глобальному
    /// `audio_config` во время playback.
    pub fn enqueue_with_outputs(
        &self,
        id: String,
        text: String,
        audio: AudioPcm,
        speaker: Option<OutputConfig>,
        mic: Option<OutputConfig>,
    ) -> bool {
        self.enqueue_inner(id, text, audio, speaker, mic)
    }

    fn enqueue_inner(
        &self,
        id: String,
        text: String,
        audio: AudioPcm,
        speaker: Option<OutputConfig>,
        mic: Option<OutputConfig>,
    ) -> bool {
        let arc_audio = Arc::new(audio);
        let mut s = self.state.write();
        match s.enqueue_state(id, text, arc_audio, speaker, mic) {
            EnqueueState::SendToThread(phrase) => {
                drop(s);
                let _ = self.cmd_tx.send(Cmd::Enqueue(phrase));
                let _ = self.app_handle.emit("queue-changed", ());
                true
            }
            EnqueueState::Queued => {
                let _ = self.app_handle.emit("queue-changed", ());
                true
            }
            EnqueueState::Rejected => {
                warn!("Playback queue full ({MAX_QUEUE}), phrase dropped");
                false
            }
        }
    }

    pub fn pause(&self) -> bool {
        if !self.state.read().can_pause() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Pause);
        true
    }

    pub fn resume(&self) -> bool {
        if !self.state.read().can_resume() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Resume);
        true
    }

    pub fn stop(&self) -> bool {
        if !self.state.read().can_stop() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Stop);
        true
    }

    pub fn repeat(&self) -> bool {
        if !self.state.read().can_repeat() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Repeat);
        true
    }

    pub fn replay_from_cache(&self, id: &str) -> Result<(), String> {
        let audio_cfg = self.audio_config.read().clone();
        let mut s = self.state.write();

        let cached = s
            .find_in_cache(id)
            .ok_or_else(|| format!("CacheMiss: no cached audio for id '{}'", id))?;
        let (_, cached_text, cached_audio) = cached;

        match s.accept_replay(
            id,
            cached_text,
            cached_audio,
            audio_cfg.speaker,
            audio_cfg.mic,
        ) {
            Ok(EnqueueState::SendToThread(phrase)) => {
                drop(s);
                let _ = self.cmd_tx.send(Cmd::Enqueue(phrase));
                let _ = self.app_handle.emit("queue-changed", ());
                Ok(())
            }
            Ok(EnqueueState::Queued) => {
                drop(s);
                let _ = self.app_handle.emit("queue-changed", ());
                Ok(())
            }
            Ok(EnqueueState::Rejected) => {
                drop(s);
                Err(format!(
                    "QueueFull: playback queue is full (max {MAX_QUEUE})"
                ))
            }
            Err(e) => Err(e),
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let s = self.state.read();
        PlaybackSnapshot {
            cache_entries: s.all_cache_entries(),
            current_id: s.current_identity().map(|(id, _)| id),
            queue_ids: s.queued_ids(),
            pb_status: s.status.clone(),
        }
    }

    pub fn cancel_queued_replay(&self, id: &str) -> Result<(), String> {
        let result = self.state.write().remove_queued_item(id);
        if result.is_ok() {
            let _ = self.app_handle.emit("queue-changed", ());
        }
        result
    }

    pub fn remove_queued_item(&self, id: &str) -> Result<(), String> {
        self.state.write().remove_queued_item(id)
    }

    pub fn has_cache_for(&self, id: &str) -> bool {
        self.state.read().has_cached_audio(id)
    }

    pub fn cache_entries_all(&self) -> Vec<(String, String, i64)> {
        self.state.read().all_cache_entries()
    }

    pub fn current_id(&self) -> Option<String> {
        self.state.read().current_identity().map(|(id, _)| id)
    }

    pub fn queued_ids(&self) -> Vec<String> {
        self.state.read().queued_ids()
    }

    pub fn on_playback_finished(&self) {
        let mut s = self.state.write();
        if let Some(next) = s.finish_inner() {
            let id = next.id.clone();
            let text = next.text.clone();
            let audio = next.audio.clone();
            let speaker = next.speaker.clone();
            let mic = next.mic.clone();
            drop(s);
            let _ = self.cmd_tx.send(Cmd::Enqueue(QueuedPhrase {
                id,
                text,
                audio,
                speaker,
                mic,
            }));
        }
    }

    pub fn get_state(&self) -> PlaybackStateDto {
        self.state.read().get_state_dto()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_audio() -> AudioPcm {
        AudioPcm {
            samples: vec![0.0_f32; 100],
            sample_rate: 24000,
            channels: 1,
        }
    }

    fn make_shared() -> Shared {
        Shared {
            status: PlaybackStatus::Idle,
            current: None,
            queue: VecDeque::new(),
            audio_cache: VecDeque::with_capacity(AUDIO_CACHE_SIZE),
        }
    }

    fn make_shared_playing() -> Shared {
        let mut s = Shared {
            status: PlaybackStatus::Playing,
            current: Some(QueuedPhrase {
                id: "current".into(),
                text: "current text".into(),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            }),
            queue: VecDeque::new(),
            audio_cache: VecDeque::with_capacity(AUDIO_CACHE_SIZE),
        };
        s.audio_cache.push_back(CachedPhrase {
            id: "current".into(),
            text: "current text".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 1000,
        });
        s
    }

    fn make_shared_paused() -> Shared {
        let mut s = Shared {
            status: PlaybackStatus::Paused,
            current: Some(QueuedPhrase {
                id: "paused_id".into(),
                text: "paused text".into(),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            }),
            queue: VecDeque::new(),
            audio_cache: VecDeque::with_capacity(AUDIO_CACHE_SIZE),
        };
        s.audio_cache.push_back(CachedPhrase {
            id: "paused_id".into(),
            text: "paused text".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 2000,
        });
        s
    }

    // ── enqueue_state ──

    #[test]
    fn enqueue_sets_current_when_idle() {
        let mut s = make_shared();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("id1".into(), "hello".into(), Arc::clone(&audio), None, None) {
            EnqueueState::SendToThread(p) => {
                assert_eq!(p.id, "id1");
                assert_eq!(p.text, "hello");
            }
            _ => panic!("expected SendToThread"),
        }
        assert_eq!(s.current.as_ref().unwrap().id, "id1");
    }

    #[test]
    fn enqueue_queues_when_playing() {
        let mut s = make_shared_playing();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("new_id".into(), "new text".into(), audio, None, None) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn enqueue_queues_when_paused() {
        let mut s = make_shared_paused();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("new_id".into(), "new text".into(), audio, None, None) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn enqueue_dedup_current_id() {
        let mut s = make_shared_playing();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("current".into(), "current text".into(), audio, None, None) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert!(s.queue.is_empty());
    }

    #[test]
    fn enqueue_dedup_already_queued() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "queued".into(),
            text: "queued text".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("queued".into(), "queued text".into(), audio, None, None) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn enqueue_queue_limit() {
        let mut s = make_shared_playing();
        for i in 0..MAX_QUEUE {
            s.queue.push_back(QueuedPhrase {
                id: format!("q{i}"),
                text: format!("text{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("over".into(), "over text".into(), audio, None, None) {
            EnqueueState::Rejected => {}
            _ => panic!("expected Rejected"),
        }
        assert_eq!(s.queue.len(), MAX_QUEUE);
    }

    #[test]
    fn enqueue_queue_fifo_order() {
        let mut s = make_shared_playing();
        for i in 0..3 {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state(format!("id{i}"), format!("text{i}"), audio, None, None);
        }
        assert_eq!(s.queue.len(), 3);
        assert_eq!(s.queue[0].id, "id0");
        assert_eq!(s.queue[1].id, "id1");
        assert_eq!(s.queue[2].id, "id2");
    }

    // ── cache ──

    #[test]
    fn cache_eviction() {
        let mut s = make_shared();
        for i in 0..(AUDIO_CACHE_SIZE + 5) {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state(format!("id{i}"), format!("text{i}"), audio, None, None);
        }
        assert_eq!(s.audio_cache.len(), AUDIO_CACHE_SIZE);
        assert_eq!(s.audio_cache[0].id, "id5");
        assert_eq!(
            s.audio_cache[AUDIO_CACHE_SIZE - 1].id,
            format!("id{}", AUDIO_CACHE_SIZE + 4)
        );
    }

    #[test]
    fn cache_dedup_refreshes_position() {
        let mut s = make_shared();
        {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state("id1".into(), "text1".into(), audio, None, None);
        }
        for i in 0..5 {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state(format!("fill{i}"), format!("text{i}"), audio, None, None);
        }
        {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state("id1".into(), "text1".into(), audio, None, None);
        }
        assert_eq!(s.audio_cache.back().unwrap().id, "id1");
        let count = s.audio_cache.iter().filter(|c| c.id == "id1").count();
        assert_eq!(count, 1);
    }

    // ── finish_inner ──

    #[test]
    fn finish_inner_pops_next_from_queue() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "next".into(),
            text: "next text".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let result = s.finish_inner();
        assert!(result.is_some());
        let next = result.unwrap();
        assert_eq!(next.id, "next");
        assert_eq!(s.current.as_ref().unwrap().id, "next");
        assert!(s.queue.is_empty());
    }

    #[test]
    fn finish_inner_empty_queue_goes_idle() {
        let mut s = make_shared_playing();
        let result = s.finish_inner();
        assert!(result.is_none());
        assert!(s.current.is_none());
        assert_eq!(s.status, PlaybackStatus::Idle);
    }

    #[test]
    fn finish_inner_multiple_preserves_order() {
        let mut s = make_shared_playing();
        for i in 0..3 {
            s.queue.push_back(QueuedPhrase {
                id: format!("q{i}"),
                text: format!("text{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        let r1 = s.finish_inner().unwrap();
        assert_eq!(r1.id, "q0");
        assert_eq!(s.current.as_ref().unwrap().id, "q0");

        let r2 = s.finish_inner().unwrap();
        assert_eq!(r2.id, "q1");

        let r3 = s.finish_inner().unwrap();
        assert_eq!(r3.id, "q2");

        let r4 = s.finish_inner();
        assert!(r4.is_none());
        assert!(s.current.is_none());
        assert_eq!(s.status, PlaybackStatus::Idle);
    }

    // ── get_state_dto ──

    #[test]
    fn get_state_dto_idle() {
        let s = make_shared();
        let dto = s.get_state_dto();
        assert_eq!(dto.status, PlaybackStatus::Idle);
        assert!(dto.current.is_none());
        assert!(dto.queue.is_empty());
        assert!(dto.recent.is_empty());
    }

    #[test]
    fn get_state_dto_playing() {
        let s = make_shared_playing();
        let dto = s.get_state_dto();
        assert_eq!(dto.status, PlaybackStatus::Playing);
        assert_eq!(dto.current.as_deref(), Some("current text"));
        assert!(dto.queue.is_empty());
    }

    #[test]
    fn get_state_dto_with_queue() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "q1".into(),
            text: "first".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        s.queue.push_back(QueuedPhrase {
            id: "q2".into(),
            text: "second".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let dto = s.get_state_dto();
        assert_eq!(dto.queue, vec!["first", "second"]);
    }

    #[test]
    fn get_state_dto_recent_last_5_reversed() {
        let mut s = make_shared();
        for i in 0..10 {
            s.audio_cache.push_back(CachedPhrase {
                id: format!("c{i}"),
                text: format!("cache{i}"),
                audio: Arc::new(dummy_audio()),
                timestamp: (1000 + i) as i64,
            });
        }
        let dto = s.get_state_dto();
        assert_eq!(dto.recent.len(), 5);
        assert_eq!(dto.recent[0].id, "c9");
        assert_eq!(dto.recent[1].id, "c8");
        assert_eq!(dto.recent[4].id, "c5");
    }

    #[test]
    fn get_state_dto_recent_less_than_5() {
        let mut s = make_shared();
        s.audio_cache.push_back(CachedPhrase {
            id: "only".into(),
            text: "only text".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 42,
        });
        let dto = s.get_state_dto();
        assert_eq!(dto.recent.len(), 1);
        assert_eq!(dto.recent[0].id, "only");
        assert_eq!(dto.recent[0].timestamp, 42);
    }

    // ── guards ──

    #[test]
    fn can_pause_no_current() {
        assert!(!make_shared().can_pause());
    }

    #[test]
    fn can_pause_with_current() {
        assert!(make_shared_playing().can_pause());
    }

    #[test]
    fn can_pause_when_paused() {
        assert!(make_shared_paused().can_pause());
    }

    #[test]
    fn can_resume_no_current() {
        assert!(!make_shared().can_resume());
    }

    #[test]
    fn can_resume_playing() {
        assert!(!make_shared_playing().can_resume());
    }

    #[test]
    fn can_resume_paused() {
        assert!(make_shared_paused().can_resume());
    }

    #[test]
    fn can_stop_no_current() {
        assert!(!make_shared().can_stop());
    }

    #[test]
    fn can_stop_with_current() {
        assert!(make_shared_playing().can_stop());
    }

    #[test]
    fn can_repeat_no_current() {
        assert!(!make_shared().can_repeat());
    }

    #[test]
    fn can_repeat_with_current() {
        assert!(make_shared_playing().can_repeat());
    }

    // ── find_in_cache ──

    #[test]
    fn find_in_cache_returns_item() {
        let mut s = make_shared();
        s.audio_cache.push_back(CachedPhrase {
            id: "find_me".into(),
            text: "found text".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 999,
        });
        let result = s.find_in_cache("find_me");
        assert!(result.is_some());
        let (id, text, _audio) = result.unwrap();
        assert_eq!(id, "find_me");
        assert_eq!(text, "found text");
    }

    #[test]
    fn find_in_cache_missing_returns_none() {
        assert!(make_shared().find_in_cache("nonexistent").is_none());
    }

    // ── per-phrase output config ──

    #[test]
    fn queued_phrases_retain_different_output_configs() {
        let cfg_a = Some(OutputConfig {
            device_id: Some("dev_a".into()),
            volume: 0.8,
        });
        let cfg_b = Some(OutputConfig {
            device_id: Some("dev_b".into()),
            volume: 0.5,
        });
        let mut s = make_shared_playing();
        let audio = Arc::new(dummy_audio());
        s.enqueue_state(
            "id_a".into(),
            "text a".into(),
            Arc::clone(&audio),
            cfg_a.clone(),
            None,
        );
        s.enqueue_state(
            "id_b".into(),
            "text b".into(),
            Arc::clone(&audio),
            cfg_b.clone(),
            Some(OutputConfig {
                device_id: Some("mic_x".into()),
                volume: 0.3,
            }),
        );
        assert_eq!(s.queue.len(), 2);
        assert_eq!(
            s.queue[0].speaker.as_ref().map(|c| &c.device_id),
            cfg_a.as_ref().map(|c| &c.device_id)
        );
        assert_eq!(
            s.queue[1].speaker.as_ref().map(|c| &c.device_id),
            cfg_b.as_ref().map(|c| &c.device_id)
        );
        assert!(s.queue[1].mic.is_some());
        assert_eq!(
            s.queue[1].mic.as_ref().and_then(|c| c.device_id.as_deref()),
            Some("mic_x")
        );
    }

    #[test]
    fn finish_inner_preserves_output_configs() {
        let mut s = make_shared_playing();
        let cfg = Some(OutputConfig {
            device_id: Some("my_dev".into()),
            volume: 0.9,
        });
        s.queue.push_back(QueuedPhrase {
            id: "next".into(),
            text: "next".into(),
            audio: Arc::new(dummy_audio()),
            speaker: cfg.clone(),
            mic: None,
        });
        let result = s.finish_inner().unwrap();
        assert_eq!(result.id, "next");
        assert_eq!(
            result.speaker.as_ref().map(|c| &c.device_id),
            cfg.as_ref().map(|c| &c.device_id)
        );
        let current = s.current.as_ref().unwrap();
        assert_eq!(
            current.speaker.as_ref().map(|c| &c.device_id),
            cfg.as_ref().map(|c| &c.device_id)
        );
    }

    #[test]
    fn enqueue_queue_limit_unchanged_with_configs() {
        let mut s = make_shared_playing();
        for i in 0..MAX_QUEUE {
            s.queue.push_back(QueuedPhrase {
                id: format!("q{i}"),
                text: format!("text{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state(
            "over".into(),
            "over text".into(),
            audio,
            Some(OutputConfig {
                device_id: Some("d".into()),
                volume: 0.5,
            }),
            None,
        ) {
            EnqueueState::Rejected => {}
            _ => panic!("expected Rejected"),
        }
        assert_eq!(s.queue.len(), MAX_QUEUE);
    }

    #[test]
    fn enqueue_state_sends_idle_with_configs() {
        let mut s = make_shared();
        let audio = Arc::new(dummy_audio());
        let cfg = Some(OutputConfig {
            device_id: Some("dev_id".into()),
            volume: 0.7,
        });
        match s.enqueue_state(
            "id1".into(),
            "hello".into(),
            Arc::clone(&audio),
            cfg.clone(),
            None,
        ) {
            EnqueueState::SendToThread(p) => {
                assert_eq!(p.id, "id1");
                assert_eq!(
                    p.speaker.as_ref().map(|c| &c.device_id),
                    cfg.as_ref().map(|c| &c.device_id)
                );
            }
            _ => panic!("expected SendToThread"),
        }
    }

    // ── cache inspection ──

    #[test]
    fn has_cached_audio_returns_true_for_item_in_cache() {
        let mut s = make_shared();
        s.audio_cache.push_back(CachedPhrase {
            id: "cached".into(),
            text: "cached text".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 42,
        });
        assert!(s.has_cached_audio("cached"));
    }

    #[test]
    fn has_cached_audio_returns_false_for_missing_item() {
        let s = make_shared();
        assert!(!s.has_cached_audio("nonexistent"));
    }

    #[test]
    fn has_cached_audio_returns_false_after_eviction() {
        let mut s = make_shared();
        for i in 0..(AUDIO_CACHE_SIZE + 5) {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state(format!("id{i}"), format!("text{i}"), audio, None, None);
        }
        assert!(!s.has_cached_audio("id0"));
        assert!(s.has_cached_audio(format!("id{}", AUDIO_CACHE_SIZE + 4).as_str()));
    }

    #[test]
    fn all_cache_entries_returns_all_items() {
        let mut s = make_shared();
        s.audio_cache.push_back(CachedPhrase {
            id: "a".into(),
            text: "text a".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 1,
        });
        s.audio_cache.push_back(CachedPhrase {
            id: "b".into(),
            text: "text b".into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 2,
        });
        let entries = s.all_cache_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "a");
        assert_eq!(entries[1].0, "b");
    }

    #[test]
    fn current_identity_returns_none_when_idle() {
        let s = make_shared();
        assert!(s.current_identity().is_none());
    }

    #[test]
    fn current_identity_returns_id_and_text_when_playing() {
        let s = make_shared_playing();
        let (id, text) = s.current_identity().unwrap();
        assert_eq!(id, "current");
        assert_eq!(text, "current text");
    }

    #[test]
    fn get_state_dto_includes_current_id() {
        let s = make_shared_playing();
        let dto = s.get_state_dto();
        assert_eq!(dto.current_id.as_deref(), Some("current"));
    }

    #[test]
    fn get_state_dto_current_id_none_when_idle() {
        let s = make_shared();
        let dto = s.get_state_dto();
        assert!(dto.current_id.is_none());
    }

    // ── find_in_cache returns audio for replay ──

    #[test]
    fn find_in_cache_returns_replayable_audio() {
        let mut s = make_shared();
        let audio = Arc::new(dummy_audio());
        s.audio_cache.push_back(CachedPhrase {
            id: "playable".into(),
            text: "playable text".into(),
            audio: Arc::clone(&audio),
            timestamp: 100,
        });
        let result = s.find_in_cache("playable");
        assert!(result.is_some());
        let (id, text, _) = result.unwrap();
        assert_eq!(id, "playable");
        assert_eq!(text, "playable text");
    }

    #[test]
    fn find_in_cache_missing_for_replay_returns_none() {
        let s = make_shared();
        assert!(s.find_in_cache("missing").is_none());
    }

    // ── queued_ids ──

    #[test]
    fn queued_ids_returns_all_queue_ids() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "q1".into(),
            text: "first".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        s.queue.push_back(QueuedPhrase {
            id: "q2".into(),
            text: "second".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let ids = s.queued_ids();
        assert_eq!(ids, vec!["q1", "q2"]);
    }

    #[test]
    fn queued_ids_empty_when_nothing_queued() {
        let s = make_shared();
        assert!(s.queued_ids().is_empty());
    }

    // ── project_playback_activity ──

    use crate::speech_queue::{JobDto, JobStatus};

    fn job_dto(job_id: &str, status: JobStatus, text: &str, last_activity_at_ms: i64) -> JobDto {
        JobDto {
            job_id: uuid::Uuid::parse_str(job_id).unwrap(),
            original_text: text.to_string(),
            spoken_text: None,
            status,
            error: None,
            attempt: 1,
            created_at_ms: last_activity_at_ms,
            last_activity_at_ms,
        }
    }

    #[test]
    fn project_job_cache_dedup() {
        let jobs = vec![job_dto(
            "11111111-1111-1111-1111-111111111111",
            JobStatus::Completed,
            "hello",
            1000,
        )];
        let cache: Vec<(String, String, i64)> = vec![(
            "11111111-1111-1111-1111-111111111111".into(),
            "hello".into(),
            1000,
        )];
        let rows = project_playback_activity(&jobs, &cache, &None, &[], &PlaybackStatus::Idle);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "11111111-1111-1111-1111-111111111111");
        assert!(rows[0].job_id.is_some());
    }

    #[test]
    fn project_completed_replay_current_playing() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "playing");
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_completed_replay_current_paused() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Paused,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "paused");
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_completed_replay_in_tail_queued() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &None,
            &[id.to_string()],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "replay_queued");
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_playback_only_current_row() {
        let cache_id = "cache-only-1";
        let cache: Vec<(String, String, i64)> = vec![(cache_id.into(), "cached".into(), 2000)];
        let rows = project_playback_activity(
            &[],
            &cache,
            &Some(cache_id.to_string()),
            &[],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "playing");
        assert!(rows[0].job_id.is_none());
    }

    #[test]
    fn project_playback_only_pending_row() {
        let cache_id = "cache-only-q";
        let cache: Vec<(String, String, i64)> = vec![(cache_id.into(), "cached".into(), 2000)];
        let rows = project_playback_activity(
            &[],
            &cache,
            &None,
            &[cache_id.to_string()],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "replay_queued");
        assert!(rows[0].job_id.is_none());
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_replay_unavailable_current() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Playing,
        );
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_replay_unavailable_queued() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &None,
            &[id.to_string()],
            &PlaybackStatus::Idle,
        );
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_replay_available_not_in_cache() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let rows = project_playback_activity(&jobs, &[], &None, &[], &PlaybackStatus::Idle);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].can_replay, "no cache means no replay");
    }

    #[test]
    fn project_replay_available_cached_completed() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(&jobs, &cache, &None, &[], &PlaybackStatus::Idle);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].can_replay);
        assert_eq!(rows[0].status, "completed");
    }

    #[test]
    fn project_replay_tail_order_independent_of_blocked_queue() {
        let completed_id = "11111111-1111-1111-1111-111111111111";
        let failed_id = "22222222-2222-2222-2222-222222222222";
        let queued_id = "33333333-3333-3333-3333-333333333333";
        let replay_id = "44444444-4444-4444-4444-444444444444";

        let jobs = vec![
            job_dto(failed_id, JobStatus::Failed, "failed job", 500),
            job_dto(queued_id, JobStatus::Queued, "queued job", 1000),
            job_dto(completed_id, JobStatus::Completed, "completed job", 1500),
        ];
        let cache: Vec<(String, String, i64)> = vec![
            (replay_id.into(), "replay text".into(), 2000),
            (completed_id.into(), "completed job".into(), 1500),
        ];

        let rows = project_playback_activity(
            &jobs,
            &cache,
            &None,
            &[replay_id.to_string()],
            &PlaybackStatus::Playing,
        );

        // replay should be in result as replay_queued
        let replay_row = rows.iter().find(|r| r.id == replay_id).unwrap();
        assert_eq!(replay_row.status, "replay_queued");
        assert!(!replay_row.can_replay);

        // blocked job (failed) still present with its status
        let failed_row = rows.iter().find(|r| r.id == failed_id).unwrap();
        assert_eq!(failed_row.status, "failed");
    }

    #[test]
    fn project_millisecond_ordering_sorts_desc() {
        let id_a = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let id_b = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        let jobs = vec![
            job_dto(id_a, JobStatus::Completed, "older", 1000),
            job_dto(id_b, JobStatus::Completed, "newer", 2000),
        ];
        let cache: Vec<(String, String, i64)> = vec![
            (id_a.into(), "older".into(), 1000),
            (id_b.into(), "newer".into(), 2000),
        ];
        let rows = project_playback_activity(&jobs, &cache, &None, &[], &PlaybackStatus::Idle);
        assert_eq!(rows[0].id, id_b);
        assert_eq!(rows[1].id, id_a);
    }

    #[test]
    fn project_cache_only_evicted_row_disappears() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let rows = project_playback_activity(
            &jobs,
            &[], // no cache entries
            &None,
            &[],
            &PlaybackStatus::Idle,
        );
        // job still present, can_replay is false because not in cache
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].can_replay);
    }

    // ── accept_replay (atomic replay core) ──

    fn phrase_for_replay(id: &str) -> (String, Arc<AudioPcm>) {
        (id.to_string(), Arc::new(dummy_audio()))
    }

    fn shared_with_cache(id: &str, ts: i64) -> Shared {
        let mut s = make_shared();
        s.audio_cache.push_back(CachedPhrase {
            id: id.to_string(),
            text: id.to_string(),
            audio: Arc::new(dummy_audio()),
            timestamp: ts,
        });
        s
    }

    #[test]
    fn accept_replay_cache_miss() {
        let mut s = make_shared();
        let (text, audio) = phrase_for_replay("no_cache");
        let result = s.accept_replay("no_cache", text, audio, None, None);
        match result {
            Err(msg) => assert!(msg.contains("CacheMiss"), "expected CacheMiss, got: {msg}"),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_already_pending_when_playing_current() {
        let mut s = make_shared_playing();
        let (text, audio) = phrase_for_replay("current");
        let result = s.accept_replay("current", text, audio, None, None);
        match result {
            Err(msg) => assert!(
                msg.contains("AlreadyPending"),
                "expected AlreadyPending, got: {msg}"
            ),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_already_pending_when_paused_current() {
        let mut s = make_shared_paused();
        let (text, audio) = phrase_for_replay("paused_id");
        let result = s.accept_replay("paused_id", text, audio, None, None);
        match result {
            Err(msg) => assert!(
                msg.contains("AlreadyPending"),
                "expected AlreadyPending, got: {msg}"
            ),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_already_pending_when_queued() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "queued".into(),
            text: "queued".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let (text, audio) = phrase_for_replay("queued");
        let result = s.accept_replay("queued", text, audio, None, None);
        match result {
            Err(msg) => assert!(
                msg.contains("AlreadyPending"),
                "expected AlreadyPending, got: {msg}"
            ),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_queue_full_when_playing() {
        let mut s = make_shared_playing();
        // populate cache for both current and new ids
        let cache_id = "new_replay";
        s.audio_cache.push_back(CachedPhrase {
            id: cache_id.into(),
            text: cache_id.into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 0,
        });
        // fill queue to max
        for i in 0..MAX_QUEUE {
            let qid = format!("q{i}");
            s.audio_cache.push_back(CachedPhrase {
                id: qid.clone(),
                text: qid.clone(),
                audio: Arc::new(dummy_audio()),
                timestamp: 0,
            });
            s.queue.push_back(QueuedPhrase {
                id: qid,
                text: format!("text{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        let (text, audio) = phrase_for_replay(cache_id);
        let result = s.accept_replay(cache_id, text, audio, None, None);
        match result {
            Err(msg) => assert!(msg.contains("QueueFull"), "expected QueueFull, got: {msg}"),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_appends_to_tail_when_playing() {
        let mut s = make_shared_playing();
        // fill some queue entries
        for i in 0..3 {
            let qid = format!("existing{i}");
            s.audio_cache.push_back(CachedPhrase {
                id: qid.clone(),
                text: qid.clone(),
                audio: Arc::new(dummy_audio()),
                timestamp: 0,
            });
            s.queue.push_back(QueuedPhrase {
                id: qid,
                text: format!("text{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        let replay_id = "replay_tail";
        s.audio_cache.push_back(CachedPhrase {
            id: replay_id.into(),
            text: replay_id.into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 0,
        });
        let (text, audio) = phrase_for_replay(replay_id);
        let result = s.accept_replay(replay_id, text, audio, None, None);
        assert!(
            matches!(result, Ok(EnqueueState::Queued)),
            "expected Queued"
        );
        assert_eq!(s.queue.len(), 4);
        assert_eq!(s.queue[3].id, replay_id);
    }

    #[test]
    fn accept_replay_becomes_current_when_idle() {
        let mut s = shared_with_cache("idle_replay", 100);
        let (text, audio) = phrase_for_replay("idle_replay");
        let result = s.accept_replay("idle_replay", text, audio, None, None);
        assert!(
            matches!(result, Ok(EnqueueState::SendToThread(_))),
            "expected SendToThread"
        );
        assert_eq!(s.current.as_ref().unwrap().id, "idle_replay");
    }

    #[test]
    fn accept_replay_becomes_current_when_stopped() {
        let mut s = make_shared_playing();
        s.status = PlaybackStatus::Stopped;
        // Stopped with current — should still be replayable
        let new_id = "stopped_replay";
        s.audio_cache.push_back(CachedPhrase {
            id: new_id.into(),
            text: new_id.into(),
            audio: Arc::new(dummy_audio()),
            timestamp: 100,
        });
        let (text, audio) = phrase_for_replay(new_id);
        let result = s.accept_replay(new_id, text, audio, None, None);
        assert!(
            matches!(result, Ok(EnqueueState::SendToThread(_))),
            "expected SendToThread (Stopped is not pending), got: {:?}",
            result
        );
        assert_eq!(s.current.as_ref().unwrap().id, new_id);
    }

    #[test]
    fn accept_replay_updates_timestamp_monotonically() {
        let mut s = shared_with_cache("ts_replay", 5000);
        s.accept_replay(
            "ts_replay",
            "ts_replay".into(),
            Arc::new(dummy_audio()),
            None,
            None,
        )
        .unwrap();
        let entry = s.audio_cache.iter().find(|c| c.id == "ts_replay").unwrap();
        assert!(entry.timestamp >= 5000, "timestamp should not decrease");
    }

    #[test]
    fn accept_replay_does_not_decrease_timestamp() {
        let mut s = shared_with_cache("future_replay", 9999999999999_i64);
        s.accept_replay(
            "future_replay",
            "future_replay".into(),
            Arc::new(dummy_audio()),
            None,
            None,
        )
        .unwrap();
        let entry = s
            .audio_cache
            .iter()
            .find(|c| c.id == "future_replay")
            .unwrap();
        assert_eq!(
            entry.timestamp, 10000000000000_i64,
            "timestamp must be strictly greater than previous"
        );
    }

    #[test]
    fn accept_replay_tail_already_pending_when_stopped() {
        let mut s = make_shared_playing();
        s.status = PlaybackStatus::Stopped;
        s.queue.push_back(QueuedPhrase {
            id: "queued".into(),
            text: "queued".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let (text, audio) = phrase_for_replay("queued");
        let result = s.accept_replay("queued", text, audio, None, None);
        match result {
            Err(msg) => assert!(
                msg.contains("AlreadyPending"),
                "expected AlreadyPending while Stopped, got: {msg}"
            ),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn accept_replay_strict_timestamp_bump_when_clock_not_ahead() {
        let previous = Utc::now().timestamp_millis() + 60_000;
        let mut s = shared_with_cache("same_ms", previous);
        s.accept_replay(
            "same_ms",
            "same_ms".into(),
            Arc::new(dummy_audio()),
            None,
            None,
        )
        .unwrap();
        let entry = s.audio_cache.iter().find(|c| c.id == "same_ms").unwrap();
        assert_eq!(
            entry.timestamp,
            previous + 1,
            "replay must advance activity when wall clock is not ahead"
        );
    }

    // ── remove_queued_item ──

    #[test]
    fn cancel_queued_replay_removes_requested_id() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "to_cancel".into(),
            text: "to_cancel".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        s.queue.push_back(QueuedPhrase {
            id: "keep_me".into(),
            text: "keep_me".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        assert_eq!(s.queue.len(), 2);
        assert!(s.remove_queued_item("to_cancel").is_ok());
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].id, "keep_me");
    }

    #[test]
    fn cancel_queued_replay_preserves_remaining_order() {
        let mut s = make_shared_playing();
        for i in 0..4 {
            s.queue.push_back(QueuedPhrase {
                id: format!("q{i}"),
                text: format!("q{i}"),
                audio: Arc::new(dummy_audio()),
                speaker: None,
                mic: None,
            });
        }
        assert!(s.remove_queued_item("q1").is_ok());
        assert_eq!(s.queue.len(), 3);
        assert_eq!(s.queue[0].id, "q0");
        assert_eq!(s.queue[1].id, "q2");
        assert_eq!(s.queue[2].id, "q3");
    }

    #[test]
    fn cancel_queued_replay_first_entry_preserves_order() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "q0".into(),
            text: "q0".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        s.queue.push_back(QueuedPhrase {
            id: "q1".into(),
            text: "q1".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        assert!(s.remove_queued_item("q0").is_ok());
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].id, "q1");
    }

    #[test]
    fn cancel_queued_replay_last_entry_preserves_order() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "q0".into(),
            text: "q0".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        s.queue.push_back(QueuedPhrase {
            id: "q1".into(),
            text: "q1".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        assert!(s.remove_queued_item("q1").is_ok());
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].id, "q0");
    }

    #[test]
    fn cancel_queued_replay_rejects_unknown_id() {
        let mut s = make_shared_playing();
        match s.remove_queued_item("nonexistent") {
            Err(msg) => assert!(msg.contains("NotFound"), "expected NotFound, got: {msg}"),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn cancel_queued_replay_rejects_current_id() {
        let mut s = make_shared_playing();
        match s.remove_queued_item("current") {
            Err(msg) => assert!(msg.contains("NotQueued"), "expected NotQueued, got: {msg}"),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn cancel_queued_replay_does_not_alter_current() {
        let mut s = make_shared_playing();
        s.queue.push_back(QueuedPhrase {
            id: "to_cancel".into(),
            text: "to_cancel".into(),
            audio: Arc::new(dummy_audio()),
            speaker: None,
            mic: None,
        });
        let current_before = s.current.as_ref().unwrap().id.clone();
        assert!(s.remove_queued_item("to_cancel").is_ok());
        assert_eq!(s.current.as_ref().unwrap().id, current_before);
        assert!(s.current.is_some());
    }

    #[test]
    fn cancel_queued_replay_from_empty_queue_rejects() {
        let mut s = make_shared_playing();
        assert!(s.queue.is_empty());
        match s.remove_queued_item("anything") {
            Err(msg) => assert!(
                msg.contains("NotFound"),
                "expected NotFound from empty queue, got: {msg}"
            ),
            _ => panic!("expected Err"),
        }
    }

    // ── Stopped projection ──

    #[test]
    fn project_stopped_job_row_shows_stopped() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Stopped,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "stopped");
        assert!(
            rows[0].can_replay,
            "stopped current with cache should allow replay"
        );
        assert!(rows[0].is_current);
    }

    #[test]
    fn project_stopped_job_playing_status_shows_stopped() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Playing, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Stopped,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "stopped");
        assert!(rows[0].can_replay, "stopped with cache should allow replay");
    }

    #[test]
    fn project_stopped_playback_only_row_shows_stopped() {
        let cache_id = "cache-only-stopped";
        let cache: Vec<(String, String, i64)> = vec![(cache_id.into(), "cached".into(), 2000)];
        let rows = project_playback_activity(
            &[],
            &cache,
            &Some(cache_id.to_string()),
            &[],
            &PlaybackStatus::Stopped,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "stopped");
        assert!(
            rows[0].can_replay,
            "stopped current playback-only row with cache should allow replay"
        );
        assert!(rows[0].job_id.is_none());
    }

    #[test]
    fn project_stopped_completed_not_current_shows_completed() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(&jobs, &cache, &None, &[], &PlaybackStatus::Stopped);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "completed");
        assert!(rows[0].can_replay);
    }

    #[test]
    fn project_stopped_replay_unavailable_in_tail() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Completed, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &None,
            &[id.to_string()],
            &PlaybackStatus::Stopped,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "replay_queued");
        assert!(
            !rows[0].can_replay,
            "ID in queue_ids must have can_replay=false even when Stopped"
        );
    }

    // ── Ready projection (restored ready-cancelled items) ──

    #[test]
    fn project_ready_current_playing_shows_playing() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Ready, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "playing");
        assert!(rows[0].is_current);
    }

    #[test]
    fn project_ready_current_paused_shows_paused() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Ready, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Paused,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "paused");
    }

    #[test]
    fn project_ready_in_tail_shows_replay_queued() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Ready, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &None,
            &[id.to_string()],
            &PlaybackStatus::Playing,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "replay_queued");
        assert!(!rows[0].can_replay);
    }

    #[test]
    fn project_ready_not_current_not_queued_shows_ready() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Ready, "hello", 1000)];
        let rows = project_playback_activity(&jobs, &[], &None, &[], &PlaybackStatus::Idle);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "ready");
    }

    #[test]
    fn project_ready_current_stopped_shows_stopped() {
        let id = "11111111-1111-1111-1111-111111111111";
        let jobs = vec![job_dto(id, JobStatus::Ready, "hello", 1000)];
        let cache: Vec<(String, String, i64)> = vec![(id.into(), "hello".into(), 1000)];
        let rows = project_playback_activity(
            &jobs,
            &cache,
            &Some(id.to_string()),
            &[],
            &PlaybackStatus::Stopped,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "stopped");
    }
}
