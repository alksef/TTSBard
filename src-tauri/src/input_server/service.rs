use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use uuid::Uuid;

use super::{
    IncomingSettings, IncomingTextItem, InputServerError, InputServerSettings, InputServerStatus,
    INBOX_CAPACITY,
};
use crate::speech_queue::SubmissionSource;

/// Result of a failed [`InputServerService::consume`] transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumeError<E> {
    /// No pending item matched the requested id; the closure was not invoked.
    UnknownId,
    /// The acceptance closure rejected the item; it was left in place.
    Rejected(E),
}

/// Backend-owned domain state for the external text input feature.
///
/// Owns the desired settings, the runtime lifecycle status, a bounded
/// pending-review FIFO and a lightweight wake channel for the future
/// supervisor. It does not open a listener or deliver text anywhere yet.
pub struct InputServerService {
    pub settings: Arc<tokio::sync::RwLock<InputServerSettings>>,
    /// In-memory Incoming policy snapshot read by the shared intake seam.
    ///
    /// Seeded from the persisted top-level `incoming` settings at setup and
    /// kept in sync by `save_incoming_settings`. It lives beside the inbox on
    /// this service for this slice so that Server and OCR intake observe the
    /// same source-neutral policy without touching the input-server settings.
    pub incoming: Arc<tokio::sync::RwLock<IncomingSettings>>,
    status: Arc<Mutex<InputServerStatus>>,
    inbox: Mutex<VecDeque<IncomingTextItem>>,
    wake_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<()>>>>,
    /// In-memory run request for this app run only. Seeded from the persisted
    /// `start_on_boot` preference at setup and never persisted itself.
    run_request: Arc<Mutex<bool>>,
}

impl InputServerService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(InputServerSettings::default())),
            incoming: Arc::new(tokio::sync::RwLock::new(IncomingSettings::default())),
            status: Arc::new(Mutex::new(InputServerStatus::Stopped)),
            inbox: Mutex::new(VecDeque::with_capacity(INBOX_CAPACITY)),
            wake_sender: Arc::new(Mutex::new(None)),
            run_request: Arc::new(Mutex::new(false)),
        }
    }

    /// Whether the listener supervisor should run in this app run.
    pub fn run_requested(&self) -> bool {
        *self.run_request.lock()
    }

    /// Update the in-memory run request. Does not touch persisted settings.
    pub fn set_run_request(&self, requested: bool) {
        *self.run_request.lock() = requested;
    }

    pub fn status(&self) -> InputServerStatus {
        self.status.lock().clone()
    }

    /// Replace the runtime status, returning whether it actually changed.
    ///
    /// The application supervisor calls this through [`Self::publish_status`]
    /// so the frontend only observes real transitions.
    pub fn set_status(&self, status: InputServerStatus) -> bool {
        let mut guard = self.status.lock();
        if *guard == status {
            return false;
        }
        *guard = status;
        true
    }

    /// Set the runtime status and invoke `emit` with the new value only when it
    /// actually changed.
    ///
    /// The emitter is a plain callback so this helper stays testable without a
    /// live Tauri app; the supervisor forwards it to
    /// `app_handle.emit("input-server-status-changed", status)`.
    pub fn publish_status<E>(&self, status: InputServerStatus, emit: E)
    where
        E: FnOnce(&InputServerStatus),
    {
        if self.set_status(status.clone()) {
            emit(&status);
        }
    }

    /// Installed by the listener supervisor before its lifecycle loop so that
    /// settings changes wake it even while the server is disabled.
    pub fn install_wake_sender(&self, sender: tokio::sync::mpsc::UnboundedSender<()>) {
        *self.wake_sender.lock() = Some(sender);
    }

    pub fn wake(&self) {
        if let Some(ref sender) = *self.wake_sender.lock() {
            let _ = sender.send(());
        }
    }

    /// Return all pending items in FIFO order without modifying the inbox.
    pub fn pending_items(&self) -> Vec<IncomingTextItem> {
        self.inbox.lock().iter().cloned().collect()
    }

    /// Enqueue a new item, normalising the text by trimming outer whitespace.
    ///
    /// Blank text and a full inbox are rejected with distinct typed errors.
    /// The producer `source` is stored on the pending item so a later approval
    /// can submit the speech job with the same producer. Editor route prefixes
    /// are intentionally not parsed here.
    pub fn enqueue(
        &self,
        text: &str,
        source: SubmissionSource,
    ) -> Result<IncomingTextItem, InputServerError> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(InputServerError::BlankText);
        }

        let mut inbox = self.inbox.lock();
        if inbox.len() >= INBOX_CAPACITY {
            return Err(InputServerError::InboxFull);
        }

        let item = IncomingTextItem {
            id: Uuid::new_v4().to_string(),
            text,
            source,
        };
        inbox.push_back(item.clone());
        Ok(item)
    }

    /// Remove and return exactly one item by id, or `UnknownId` if absent.
    pub fn take(&self, id: &str) -> Result<IncomingTextItem, InputServerError> {
        let mut inbox = self.inbox.lock();
        match inbox.iter().position(|item| item.id == id) {
            Some(position) => Ok(inbox.remove(position).expect("position points to an item")),
            None => Err(InputServerError::UnknownId),
        }
    }

    /// Remove exactly one item by id, or `UnknownId` if absent.
    pub fn discard(&self, id: &str) -> Result<(), InputServerError> {
        let mut inbox = self.inbox.lock();
        match inbox.iter().position(|item| item.id == id) {
            Some(position) => {
                inbox.remove(position);
                Ok(())
            }
            None => Err(InputServerError::UnknownId),
        }
    }

    /// Transactionally accept and remove one item by id.
    ///
    /// Owns the inbox lock across the lookup, the caller-supplied synchronous
    /// acceptance closure, and conditional removal. For an unknown id returns
    /// [`ConsumeError::UnknownId`] without invoking the closure. If the closure
    /// succeeds exactly the matched item is removed and its result returned; if
    /// the closure fails its error is returned and the item stays at its
    /// original FIFO position.
    pub fn consume<R, E>(
        &self,
        id: &str,
        accept: impl FnOnce(&IncomingTextItem) -> Result<R, E>,
    ) -> Result<R, ConsumeError<E>> {
        let mut inbox = self.inbox.lock();
        let position = inbox
            .iter()
            .position(|item| item.id == id)
            .ok_or(ConsumeError::UnknownId)?;
        let result = accept(&inbox[position]).map_err(ConsumeError::Rejected)?;
        inbox.remove(position);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConsumeError, InputServerService};
    use crate::input_server::{
        IncomingSettings, IncomingTextItem, InputServerError, InputServerSettings,
        InputServerStatus, INBOX_CAPACITY,
    };
    use crate::speech_queue::SubmissionSource;

    #[test]
    fn settings_defaults() {
        let settings = InputServerSettings::default();
        assert!(!settings.start_on_boot);
        assert_eq!(settings.port, 10101);
    }

    #[test]
    fn service_starts_stopped() {
        let service = InputServerService::new();
        assert_eq!(service.status(), InputServerStatus::Stopped);
        assert!(service.pending_items().is_empty());
    }

    #[test]
    fn set_status_updates_status() {
        let service = InputServerService::new();
        assert!(service.set_status(InputServerStatus::Running));
        assert_eq!(service.status(), InputServerStatus::Running);
    }

    #[test]
    fn set_status_reports_unchanged_transition_as_false() {
        let service = InputServerService::new();
        assert!(service.set_status(InputServerStatus::Running));
        assert!(!service.set_status(InputServerStatus::Running));
        assert_eq!(service.status(), InputServerStatus::Running);
    }

    #[test]
    fn publish_status_emits_only_when_status_changes() {
        let service = InputServerService::new();
        let mut emitted: Vec<InputServerStatus> = Vec::new();

        service.publish_status(InputServerStatus::Stopped, |status| {
            emitted.push(status.clone());
        });
        assert!(emitted.is_empty(), "no emit when status did not change");

        service.publish_status(InputServerStatus::Starting, |status| {
            emitted.push(status.clone());
        });
        service.publish_status(InputServerStatus::Starting, |status| {
            emitted.push(status.clone());
        });
        service.publish_status(
            InputServerStatus::Error {
                message: "boom".into(),
            },
            |status| {
                emitted.push(status.clone());
            },
        );

        assert_eq!(
            emitted,
            vec![
                InputServerStatus::Starting,
                InputServerStatus::Error {
                    message: "boom".into()
                },
            ]
        );
        assert_eq!(
            service.status(),
            InputServerStatus::Error {
                message: "boom".into()
            }
        );
    }

    #[test]
    fn service_settings_use_expected_defaults() {
        let service = InputServerService::new();
        let settings = service.settings.blocking_read();
        assert!(!settings.start_on_boot);
        assert_eq!(settings.port, 10101);
    }

    #[test]
    fn incoming_snapshot_defaults_auto_play_true() {
        let service = InputServerService::new();
        let incoming = service.incoming.blocking_read();
        assert!(incoming.auto_play);
        assert_eq!(*incoming, IncomingSettings::default());
    }

    #[test]
    fn incoming_snapshot_updates_without_touching_server_settings() {
        let service = InputServerService::new();
        service.settings.blocking_write().start_on_boot = true;

        {
            let mut incoming = service.incoming.blocking_write();
            incoming.auto_play = false;
        }

        let settings = service.settings.blocking_read();
        assert!(settings.start_on_boot, "server settings must be untouched");
        assert!(!service.incoming.blocking_read().auto_play);
    }

    #[test]
    fn enqueue_trims_outer_whitespace() {
        let service = InputServerService::new();
        let item = service
            .enqueue("  hello world  ", SubmissionSource::Server)
            .unwrap();
        assert_eq!(item.text, "hello world");
        assert_eq!(service.pending_items()[0].text, "hello world");
    }

    #[test]
    fn enqueue_rejects_blank_text() {
        let service = InputServerService::new();
        assert_eq!(
            service.enqueue("", SubmissionSource::Server),
            Err(InputServerError::BlankText)
        );
        assert_eq!(
            service.enqueue("   ", SubmissionSource::Server),
            Err(InputServerError::BlankText)
        );
        assert_eq!(
            service.enqueue("\t\n", SubmissionSource::Server),
            Err(InputServerError::BlankText)
        );
        assert!(service.pending_items().is_empty());
    }

    #[test]
    fn pending_items_are_fifo_ordered() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();
        let third = service.enqueue("three", SubmissionSource::Server).unwrap();

        let items = service.pending_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[1].id, second.id);
        assert_eq!(items[2].id, third.id);
    }

    #[test]
    fn enqueue_rejects_full_inbox() {
        let service = InputServerService::new();
        for index in 0..INBOX_CAPACITY {
            service
                .enqueue(&format!("item {index}"), SubmissionSource::Server)
                .unwrap();
        }

        assert_eq!(
            service.enqueue("overflow", SubmissionSource::Server),
            Err(InputServerError::InboxFull)
        );
        assert_eq!(service.pending_items().len(), INBOX_CAPACITY);
    }

    #[test]
    fn take_removes_exactly_one_item() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();

        assert_eq!(service.take(&first.id).unwrap(), first);

        let items = service.pending_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, second.id);
    }

    #[test]
    fn discard_removes_exactly_one_item() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();

        assert_eq!(service.discard(&first.id), Ok(()));

        let items = service.pending_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, second.id);
    }

    #[test]
    fn take_unknown_id_is_error_and_preserves_fifo() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();

        assert_eq!(service.take("missing"), Err(InputServerError::UnknownId));

        let items = service.pending_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[1].id, second.id);
    }

    #[test]
    fn discard_unknown_id_is_error_and_preserves_fifo() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();

        assert_eq!(service.discard("missing"), Err(InputServerError::UnknownId));

        let items = service.pending_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id);
    }

    #[test]
    fn consume_success_removes_exactly_one_item() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();

        let accepted = service
            .consume(&first.id, |item| {
                assert_eq!(item.id, first.id);
                Ok::<String, ()>(item.text.clone())
            })
            .unwrap();

        assert_eq!(accepted, "one");
        let items = service.pending_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, second.id);
    }

    #[test]
    fn consume_rejection_preserves_fifo_position() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();
        let second = service.enqueue("two", SubmissionSource::Server).unwrap();
        let third = service.enqueue("three", SubmissionSource::Server).unwrap();

        let result = service.consume(&second.id, |_item| Err::<(), _>("boom"));
        assert_eq!(result, Err(ConsumeError::Rejected("boom")));

        let items = service.pending_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[1].id, second.id);
        assert_eq!(items[2].id, third.id);
    }

    #[test]
    fn consume_unknown_id_does_not_call_closure() {
        let service = InputServerService::new();
        let first = service.enqueue("one", SubmissionSource::Server).unwrap();

        let mut called = false;
        let result = service.consume("missing", |_item| {
            called = true;
            Ok::<(), ()>(())
        });

        assert_eq!(result, Err(ConsumeError::UnknownId));
        assert!(!called);
        let items = service.pending_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id);
    }

    #[test]
    fn consume_second_attempt_after_accept_is_unknown_id() {
        let service = InputServerService::new();
        let item = service.enqueue("one", SubmissionSource::Server).unwrap();

        let mut calls = 0;
        let first = service.consume(&item.id, |_item| {
            calls += 1;
            Ok::<(), ()>(())
        });
        assert_eq!(first, Ok(()));

        let second = service.consume(&item.id, |_item| {
            calls += 1;
            Ok::<(), ()>(())
        });
        assert_eq!(second, Err(ConsumeError::UnknownId));

        assert_eq!(calls, 1);
        assert!(service.pending_items().is_empty());
    }

    #[test]
    fn status_wire_shape_matches_webview_convention() {
        assert_eq!(
            serde_json::to_value(InputServerStatus::Stopped).unwrap(),
            serde_json::json!({ "state": "stopped" })
        );
        assert_eq!(
            serde_json::to_value(InputServerStatus::Starting).unwrap(),
            serde_json::json!({ "state": "starting" })
        );
        assert_eq!(
            serde_json::to_value(InputServerStatus::Running).unwrap(),
            serde_json::json!({ "state": "running" })
        );
        assert_eq!(
            serde_json::to_value(InputServerStatus::Error {
                message: "boom".to_string()
            })
            .unwrap(),
            serde_json::json!({ "state": "error", "message": "boom" })
        );
    }

    #[test]
    fn enqueue_stores_producer_source_on_pending_item() {
        let service = InputServerService::new();
        let server_item = service
            .enqueue("server text", SubmissionSource::Server)
            .unwrap();
        assert_eq!(server_item.source, SubmissionSource::Server);

        let ocr_item = service.enqueue("ocr text", SubmissionSource::Ocr).unwrap();
        assert_eq!(ocr_item.source, SubmissionSource::Ocr);

        let items = service.pending_items();
        assert_eq!(items[0].source, SubmissionSource::Server);
        assert_eq!(items[1].source, SubmissionSource::Ocr);
    }

    #[test]
    fn pending_source_survives_take() {
        let service = InputServerService::new();
        let item = service.enqueue("survives", SubmissionSource::Ocr).unwrap();
        let taken = service.take(&item.id).unwrap();
        assert_eq!(taken.source, SubmissionSource::Ocr);
    }

    #[test]
    fn consume_hands_stored_source_to_approval_closure() {
        let service = InputServerService::new();
        let server_item = service
            .enqueue("approve server", SubmissionSource::Server)
            .unwrap();
        let ocr_item = service
            .enqueue("approve ocr", SubmissionSource::Ocr)
            .unwrap();

        let seen_server = service
            .consume(&server_item.id, |pending| {
                Ok::<SubmissionSource, ()>(pending.source)
            })
            .unwrap();
        assert_eq!(seen_server, SubmissionSource::Server);

        let seen_ocr = service
            .consume(&ocr_item.id, |pending| {
                Ok::<SubmissionSource, ()>(pending.source)
            })
            .unwrap();
        assert_eq!(seen_ocr, SubmissionSource::Ocr);
    }

    #[test]
    fn incoming_item_wire_shape_is_snake_case() {
        let item = IncomingTextItem {
            id: "abc".to_string(),
            text: "hello".to_string(),
            source: SubmissionSource::Server,
        };
        assert_eq!(
            serde_json::to_value(item).unwrap(),
            serde_json::json!({ "id": "abc", "text": "hello", "source": "server" })
        );

        let ocr_item = IncomingTextItem {
            id: "def".to_string(),
            text: "bonjour".to_string(),
            source: SubmissionSource::Ocr,
        };
        assert_eq!(
            serde_json::to_value(ocr_item).unwrap(),
            serde_json::json!({ "id": "def", "text": "bonjour", "source": "ocr" })
        );
    }

    #[test]
    fn wake_signals_installed_sender() {
        let service = InputServerService::new();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        service.install_wake_sender(sender);
        service.wake();

        assert_eq!(receiver.try_recv(), Ok(()));
    }

    #[test]
    fn wake_without_sender_does_not_panic() {
        let service = InputServerService::new();
        service.wake();
    }

    #[test]
    fn run_request_defaults_false_and_tracks_updates() {
        let service = InputServerService::new();
        assert!(!service.run_requested());

        service.set_run_request(true);
        assert!(service.run_requested());

        service.set_run_request(false);
        assert!(!service.run_requested());
    }
}
