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
    pub queue: Vec<String>,
    pub recent: Vec<RecentPhrase>,
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

enum EnqueueState {
    SendToThread(QueuedPhrase),
    Queued,
    Rejected,
}

impl Shared {
    fn enqueue_state(&mut self, id: String, text: String, audio: Arc<AudioPcm>) -> EnqueueState {
        let ts = Utc::now().timestamp();
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
                self.queue.push_back(QueuedPhrase { id, text, audio });
                return EnqueueState::Queued;
            }
            return EnqueueState::Rejected;
        }

        let phrase = QueuedPhrase {
            id: id.clone(),
            text,
            audio,
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
}

pub struct PlaybackManager {
    cmd_tx: mpsc::Sender<Cmd>,
    state: Arc<RwLock<Shared>>,
    pub audio_config: Arc<RwLock<AudioOutputsConfig>>,
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

        thread::spawn(move || {
            Self::thread_loop(
                cmd_rx,
                th_cmd_tx,
                app_handle,
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
        }
    }

    fn thread_loop(
        cmd_rx: Receiver<Cmd>,
        cmd_tx: mpsc::Sender<Cmd>,
        app: AppHandle,
        internal_ev: mpsc::Sender<crate::events::AppEvent>,
        state: Arc<RwLock<Shared>>,
        audio_config: Arc<RwLock<AudioOutputsConfig>>,
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

                    // Читаем актуальную конфигурацию на каждый Enqueue (C1-дыра)
                    let cfg = audio_config.read().clone();
                    let audio = phrase.audio.clone();

                    if let Some(ref c) = cfg.speaker {
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
                    if let Some(ref c) = cfg.mic {
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
                    state.write().status = PlaybackStatus::Idle;
                    info!(target: "playback", "PlaybackFinished, playing reset");
                    let _ = internal_ev.send(crate::events::AppEvent::PlaybackFinished);
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

    /// Добавить фразу в очередь. Возвращает `true` если фраза принята, `false` если очередь полна.
    pub fn enqueue(&self, id: String, text: String, audio: AudioPcm) -> bool {
        let arc_audio = Arc::new(audio);
        let mut s = self.state.write();
        match s.enqueue_state(id, text, arc_audio) {
            EnqueueState::SendToThread(phrase) => {
                drop(s);
                let _ = self.cmd_tx.send(Cmd::Enqueue(phrase));
                true
            }
            EnqueueState::Queued => true,
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

    pub fn replay_from_cache(&self, id: &str) {
        let replay = self.state.read().find_in_cache(id);
        if let Some((id, text, audio)) = replay {
            self.enqueue(id, text, (*audio).clone());
        }
    }

    pub fn on_playback_finished(&self) {
        let mut s = self.state.write();
        if let Some(next) = s.finish_inner() {
            let id = next.id.clone();
            let text = next.text.clone();
            let audio = next.audio.clone();
            drop(s);
            let _ = self
                .cmd_tx
                .send(Cmd::Enqueue(QueuedPhrase { id, text, audio }));
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
        match s.enqueue_state("id1".into(), "hello".into(), Arc::clone(&audio)) {
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
        match s.enqueue_state("new_id".into(), "new text".into(), audio) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].id, "new_id");
        assert_eq!(s.current.as_ref().unwrap().id, "current");
    }

    #[test]
    fn enqueue_queues_when_paused() {
        let mut s = make_shared_paused();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("new_id".into(), "new text".into(), audio) {
            EnqueueState::Queued => {}
            _ => panic!("expected Queued"),
        }
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn enqueue_dedup_current_id() {
        let mut s = make_shared_playing();
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("current".into(), "current text".into(), audio) {
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
        });
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("queued".into(), "queued text".into(), audio) {
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
            });
        }
        let audio = Arc::new(dummy_audio());
        match s.enqueue_state("over".into(), "over text".into(), audio) {
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
            s.enqueue_state(format!("id{i}"), format!("text{i}"), audio);
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
            s.enqueue_state(format!("id{i}"), format!("text{i}"), audio);
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
            s.enqueue_state("id1".into(), "text1".into(), audio);
        }
        for i in 0..5 {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state(format!("fill{i}"), format!("text{i}"), audio);
        }
        {
            let audio = Arc::new(dummy_audio());
            s.enqueue_state("id1".into(), "text1".into(), audio);
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
        });
        s.queue.push_back(QueuedPhrase {
            id: "q2".into(),
            text: "second".into(),
            audio: Arc::new(dummy_audio()),
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
}
