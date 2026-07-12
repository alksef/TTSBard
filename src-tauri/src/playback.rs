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

        let ts = Utc::now().timestamp();
        // Дедуп по id: если фраза уже в кеше (например, при replay_from_cache передаётся
        // тот же id) — обновить, а не добавлять дубликат (иначе replay плодит копии в «Недавних»).
        // Обычный speak_text каждый раз создаёт новый id → дедуп его не затрагивает.
        s.audio_cache.retain(|c| c.id != id);
        s.audio_cache.push_back(CachedPhrase {
            id: id.clone(),
            text: text.clone(),
            audio: Arc::clone(&arc_audio),
            timestamp: ts,
        });
        if s.audio_cache.len() > AUDIO_CACHE_SIZE {
            s.audio_cache.pop_front();
        }

        if s.current.is_some()
            && (s.status == PlaybackStatus::Playing || s.status == PlaybackStatus::Paused)
        {
            if s.queue.len() < MAX_QUEUE {
                s.queue.push_back(QueuedPhrase {
                    id,
                    text,
                    audio: arc_audio,
                });
                return true;
            }
            warn!("Playback queue full ({MAX_QUEUE}), phrase dropped");
            return false;
        }

        let phrase = QueuedPhrase {
            id: id.clone(),
            text: text.clone(),
            audio: arc_audio,
        };
        s.current = Some(phrase.clone());
        drop(s);

        let _ = self.cmd_tx.send(Cmd::Enqueue(phrase));
        true
    }

    pub fn pause(&self) -> bool {
        if self.state.read().current.is_none() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Pause);
        true
    }

    pub fn resume(&self) -> bool {
        let s = self.state.read();
        if s.current.is_none() || s.status != PlaybackStatus::Paused {
            return false;
        }
        drop(s);
        let _ = self.cmd_tx.send(Cmd::Resume);
        true
    }

    pub fn stop(&self) -> bool {
        if self.state.read().current.is_none() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Stop);
        true
    }

    pub fn repeat(&self) -> bool {
        if self.state.read().current.is_none() {
            return false;
        }
        let _ = self.cmd_tx.send(Cmd::Repeat);
        true
    }

    pub fn replay_from_cache(&self, id: &str) {
        let replay: Option<(String, String, Arc<AudioPcm>)> = {
            let s = self.state.read();
            s.audio_cache
                .iter()
                .find(|c| c.id == id)
                .map(|c| (c.id.clone(), c.text.clone(), Arc::clone(&c.audio)))
        };
        if let Some((id, text, audio)) = replay {
            self.enqueue(id, text, (*audio).clone());
        }
    }

    pub fn on_playback_finished(&self) {
        let mut s = self.state.write();
        if let Some(next) = s.queue.pop_front() {
            let id = next.id.clone();
            let text = next.text.clone();
            s.current = Some(next.clone());
            let audio = next.audio.clone();
            drop(s);
            let _ = self
                .cmd_tx
                .send(Cmd::Enqueue(QueuedPhrase { id, text, audio }));
        } else {
            s.current = None;
            s.status = PlaybackStatus::Idle;
        }
    }

    pub fn get_state(&self) -> PlaybackStateDto {
        let s = self.state.read();
        PlaybackStateDto {
            status: s.status.clone(),
            current: s.current.as_ref().map(|p| p.text.clone()),
            queue: s.queue.iter().map(|p| p.text.clone()).collect(),
            recent: s
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
}
