//! Single-instance guard (Windows only).
//!
//! Guarantees a single running copy per Windows session. The first process
//! creates a named mutex plus a hidden top-level receiver window; any later
//! process discovers that window via `FindWindowW` and notifies it
//! (`WM_COPYDATA`) to surface the main window, then exits immediately. The
//! check runs as the very first statement of `run()`, before logging, settings
//! or managers are touched, so a second copy never writes to the log or user
//! files.
//!
//! Notification is bounded: a shared 10-second deadline covers both the retry
//! discovery of the receiver window and `SendMessageTimeoutW` delivery, which
//! also verifies the window procedure's acknowledgement. A failed or
//! unacknowledged delivery never falls through to a second working copy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tracing::info;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    GetLastError, SetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, ERROR_TIMEOUT, HINSTANCE,
    HWND, LPARAM, LRESULT, WIN32_ERROR, WPARAM,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateMutexW, Sleep};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateWindowExW, DefWindowProcW, FindWindowW, MessageBoxW,
    RegisterClassExW, SendMessageTimeoutW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    MSGFLT_ALLOW, SMTO_ABORTIFHUNG, WM_COPYDATA, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED,
};

const MUTEX_NAME: &str = "com.al.ttsbard.single-instance-mutex";
const WINDOW_CLASS: &str = "com.al.ttsbard.single-instance-wnd";
const WINDOW_NAME: &str = "com.al.ttsbard.single-instance-target";
const WM_COPYDATA_SHOW: usize = 0x54545342; // "TTSB"

/// Value the window procedure returns for an accepted show request; the sender
/// uses it as the acknowledgement.
const SHOW_ACK: usize = 1;

const MESSAGE_BOX_CAPTION: &str = "TTSBard";
const MSG_CANNOT_CONTACT: &str =
    "Не удалось связаться с уже запущенной копией TTSBard. Закройте предыдущее окно приложения и запустите снова.";
const MSG_CANNOT_INIT: &str =
    "Не удалось инициализировать проверку единственного экземпляра TTSBard.";

/// Set when a show request arrives before the callback is registered. The
/// callback registration consumes it (see `register_show_callback`).
static PENDING_SHOW: AtomicBool = AtomicBool::new(false);

/// Callback invoked when another process asks this instance to show its main
/// window. Runs on the main thread (message loop) with no contention.
static SHOW_CALLBACK: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// Bounds for readiness/discovery and delivery. Injectable so the isolated
/// process tests can run with short deadlines instead of the production 10 s.
#[derive(Clone, Copy)]
struct DeliveryTiming {
    deadline: Duration,
    discovery_interval: Duration,
}

impl Default for DeliveryTiming {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(10),
            discovery_interval: Duration::from_millis(50),
        }
    }
}

/// Why a show request could not be delivered to (or acknowledged by) the first
/// instance.
#[derive(Debug)]
enum DeliveryError {
    ReceiverNotFound,
    ReceiverUnresponsive,
    DeliveryFailed(WIN32_ERROR),
    NotAcknowledged,
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReceiverNotFound => write!(f, "receiver window not found within deadline"),
            Self::ReceiverUnresponsive => write!(f, "receiver did not acknowledge within deadline"),
            Self::DeliveryFailed(code) => write!(f, "delivery failed (last error {code:?})"),
            Self::NotAcknowledged => write!(f, "receiver did not acknowledge the request"),
        }
    }
}

/// Why the receiver window could not be created. Any of these is fatal for the
/// first instance: silently losing cross-instance activation is not allowed.
#[derive(Debug)]
enum ReceiverError {
    GetModuleHandle(String),
    RegisterClass(WIN32_ERROR),
    CreateWindow(String),
    MessageFilter(String),
}

impl std::fmt::Display for ReceiverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetModuleHandle(e) => write!(f, "GetModuleHandleW: {e}"),
            Self::RegisterClass(code) => write!(f, "RegisterClassExW (last error {code:?})"),
            Self::CreateWindow(e) => write!(f, "CreateWindowExW: {e}"),
            Self::MessageFilter(e) => write!(f, "ChangeWindowMessageFilterEx: {e}"),
        }
    }
}

/// Acquire the single-instance lock. Returns for the first instance (after
/// creating the hidden receiver window); a second instance or a fatal lock
/// error shows a native error dialog and exits the process instead of returning.
pub fn acquire_lock_or_exit() {
    let mutex = match unsafe { CreateMutexW(None, true, PCWSTR(mutf16(MUTEX_NAME).as_ptr())) } {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("TTSBard: не удалось создать single-instance mutex: {err}");
            show_message_box(MSG_CANNOT_INIT);
            std::process::exit(1);
        }
    };

    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_ALREADY_EXISTS {
        // Another copy is already running: ask it to show its window, then exit
        // unconditionally. A failed delivery is a non-zero exit and never
        // falls through to a second working copy.
        match deliver_show_request(WINDOW_CLASS, WINDOW_NAME, DeliveryTiming::default()) {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                eprintln!("TTSBard: не удалось связаться с уже запущенной копией: {err}");
                show_message_box(MSG_CANNOT_CONTACT);
                std::process::exit(1);
            }
        }
    }

    // Deliberately never close the handle: it must live for the whole process
    // so the kernel releases the mutex only on exit (that is what protects
    // against stale locks after a crash).
    let _keep_alive = mutex;

    if let Err(err) = create_receiver_window(WINDOW_CLASS, WINDOW_NAME) {
        eprintln!("TTSBard: не удалось инициализировать single-instance приёмник: {err}");
        show_message_box(MSG_CANNOT_INIT);
        std::process::exit(1);
    }
}

/// Register the callback that surfaces the main window for a second-instance
/// launch. If a show request already arrived (before this registration), it is
/// consumed and the callback is invoked immediately.
pub fn register_show_callback(callback: impl Fn() + Send + Sync + 'static) {
    let callback = Arc::new(callback);
    *SHOW_CALLBACK.lock().unwrap() = Some(callback.clone());

    if PENDING_SHOW.swap(false, Ordering::SeqCst) {
        callback();
    }
}

/// Encode `s` as a null-terminated UTF-16 buffer for Windows wide APIs.
fn mutf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Deliver a show request to the first instance's receiver window, bounded by
/// `timing.deadline`. The receiver window is created after the mutex, so it may
/// not exist on the first lookup; discovery retries until it appears or the
/// deadline passes, and delivery verifies the window procedure acknowledgement.
fn deliver_show_request(
    class_name: &str,
    window_name: &str,
    timing: DeliveryTiming,
) -> Result<(), DeliveryError> {
    let deadline = Instant::now() + timing.deadline;

    let hwnd = loop {
        if let Some(hwnd) = find_window(class_name, window_name) {
            break hwnd;
        }
        if Instant::now() >= deadline {
            return Err(DeliveryError::ReceiverNotFound);
        }
        unsafe { Sleep(timing.discovery_interval.as_millis().max(1) as u32) };
    };

    send_show(hwnd, deadline)
}

/// Find the first instance's top-level receiver window by class and title.
fn find_window(class_name: &str, window_name: &str) -> Option<HWND> {
    let class = mutf16(class_name);
    let name = mutf16(window_name);
    match unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR(name.as_ptr())) } {
        Ok(hwnd) if !hwnd.0.is_null() => Some(hwnd),
        _ => None,
    }
}

/// Send the show request synchronously with the remaining deadline budget and
/// inspect both the transport outcome and the window procedure acknowledgement.
fn send_show(hwnd: HWND, deadline: Instant) -> Result<(), DeliveryError> {
    // The payload is a stack local that stays alive for the whole synchronous
    // call below.
    let data = COPYDATASTRUCT {
        dwData: WM_COPYDATA_SHOW,
        cbData: 4,
        lpData: b"show".as_ptr() as _,
    };

    let remaining = deadline.saturating_duration_since(Instant::now());
    let timeout_ms = remaining.as_millis().min(u32::MAX as u128) as u32;

    unsafe {
        // Disambiguate a generic failure from a timeout.
        SetLastError(ERROR_SUCCESS);
        let mut result: usize = 0;
        let ret = SendMessageTimeoutW(
            hwnd,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&data as *const _ as isize),
            SMTO_ABORTIFHUNG,
            timeout_ms,
            Some(&mut result),
        );

        if ret.0 == 0 {
            let last_error = GetLastError();
            if last_error == ERROR_TIMEOUT {
                return Err(DeliveryError::ReceiverUnresponsive);
            }
            return Err(DeliveryError::DeliveryFailed(last_error));
        }

        if result != SHOW_ACK {
            return Err(DeliveryError::NotAcknowledged);
        }

        Ok(())
    }
}

/// Create the hidden top-level receiver window owned by the first instance.
/// Creation failures are fatal: the mutex is already held, so a silently
/// missing receiver would lose cross-instance activation for the whole session.
fn create_receiver_window(class_name: &str, window_name: &str) -> Result<(), ReceiverError> {
    let class_name = mutf16(class_name);
    let window_name = mutf16(window_name);

    unsafe {
        let module = GetModuleHandleW(PCWSTR::null())
            .map_err(|err| ReceiverError::GetModuleHandle(err.to_string()))?;

        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = HINSTANCE(module.0);
        wc.lpszClassName = PCWSTR(class_name.as_ptr());

        if RegisterClassExW(&wc) == 0 {
            return Err(ReceiverError::RegisterClass(GetLastError()));
        }

        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_name.as_ptr()),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        )
        .map_err(|err| ReceiverError::CreateWindow(err.to_string()))?;

        // Allow WM_COPYDATA from a lower-integrity sender (e.g. an elevated
        // first instance receiving from a non-elevated second instance).
        ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, None)
            .map_err(|err| ReceiverError::MessageFilter(err.to_string()))?;
    }

    Ok(())
}

/// Show a fatal-error dialog. Used because release builds have no console; the
/// dialog acknowledgement is deliberately outside any IPC deadline.
fn show_message_box(text: &str) {
    let text = mutf16(text);
    let caption = mutf16(MESSAGE_BOX_CAPTION);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA && lparam.0 != 0 {
        let data = &*(lparam.0 as *const COPYDATASTRUCT);
        if data.dwData == WM_COPYDATA_SHOW && data.cbData >= 4 && !data.lpData.is_null() {
            // Acknowledge the request; invoke the callback (if registered) or
            // record a pending show for `register_show_callback`. The callback
            // is cloned inside the lock and invoked outside it.
            let callback = SHOW_CALLBACK.lock().ok().and_then(|guard| guard.clone());
            if let Some(callback) = callback {
                info!("single-instance: show request received, surfacing main window");
                callback();
            } else {
                PENDING_SHOW.store(true, Ordering::SeqCst);
            }
            return LRESULT(SHOW_ACK as isize);
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::{
        create_receiver_window, deliver_show_request, mutf16, register_show_callback,
        DeliveryError, DeliveryTiming, PENDING_SHOW,
    };
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, Receiver};
    use std::time::{Duration, Instant};
    use windows::Win32::System::Threading::Sleep;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
    };

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Unique, session-scoped identities per test so the isolated processes
    /// never collide with each other or with a running TTSBard.
    fn unique_identity() -> (String, String) {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let suffix = format!("{}-{}", std::process::id(), id);
        (
            format!("ttsbard-test-class-{suffix}"),
            format!("ttsbard-test-wnd-{suffix}"),
        )
    }

    /// Owns a spawned helper child and guarantees it is reaped on drop, even if
    /// an assertion fails.
    struct ChildGuard {
        child: Child,
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// Spawn the helper child test process (this same test binary re-executed)
    /// acting as the first instance's receiver, and return a line channel over
    /// its stdout for readiness/acknowledgement markers.
    fn spawn_receiver(mode: &str, delay_ms: u32) -> (String, String, ChildGuard, Receiver<String>) {
        let (class, name) = unique_identity();
        let exe = std::env::current_exe().expect("current test executable");
        let mut child = Command::new(exe)
            .args([
                "--exact",
                "single_instance::tests::receiver_helper",
                "--nocapture",
            ])
            .env("TTSBARD_TEST_CLASS", &class)
            .env("TTSBARD_TEST_NAME", &name)
            .env("TTSBARD_TEST_MODE", mode)
            .env("TTSBARD_TEST_DELAY_MS", delay_ms.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn receiver child");

        let stdout = child.stdout.take().expect("child stdout");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        (class, name, ChildGuard { child }, rx)
    }

    fn wait_for_marker(rx: &Receiver<String>, marker: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(line) if line.contains(marker) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    }

    unsafe fn pump_available() {
        let mut msg = std::mem::zeroed::<MSG>();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).0 != 0 {
            if msg.message == WM_QUIT {
                return;
            }
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }

    unsafe fn pump_for(duration: Duration) {
        let deadline = Instant::now() + duration;
        loop {
            pump_available();
            if Instant::now() >= deadline {
                return;
            }
            Sleep(10);
        }
    }

    /// Helper child process: creates a real receiver window with the injected
    /// identities and runs a native message pump. Driven entirely by env vars so
    /// it never touches the production mutex/window names or user settings.
    #[test]
    fn receiver_helper() {
        let Ok(class) = std::env::var("TTSBARD_TEST_CLASS") else {
            return;
        };
        let name = std::env::var("TTSBARD_TEST_NAME").expect("TTSBARD_TEST_NAME");
        let mode = std::env::var("TTSBARD_TEST_MODE").unwrap_or_default();
        let delay_ms: u32 = std::env::var("TTSBARD_TEST_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        if delay_ms > 0 {
            unsafe { Sleep(delay_ms) };
        }

        create_receiver_window(&class, &name).expect("receiver window creation failed");

        println!("__READY__");
        let _ = std::io::stdout().flush();

        match mode.as_str() {
            "unresponsive" => {
                // The window exists but its thread never pumps messages, so the
                // sender's SendMessageTimeoutW must time out. Sleep until the
                // parent reaps us.
                loop {
                    unsafe { Sleep(1000) };
                }
            }
            "pending" => {
                // Pump until the show request arrives (the window procedure
                // records PENDING_SHOW and acknowledges), then register the
                // callback, which must process the pending request.
                let deadline = Instant::now() + Duration::from_secs(20);
                while !PENDING_SHOW.load(Ordering::SeqCst) {
                    assert!(Instant::now() < deadline, "show request never arrived");
                    unsafe {
                        pump_available();
                    }
                    unsafe { Sleep(5) };
                }
                register_show_callback(|| {
                    println!("__CALLBACK__");
                    let _ = std::io::stdout().flush();
                });
                unsafe {
                    pump_available();
                }
            }
            _ => {
                // "ready" (and delayed discovery, via delay_ms): pump until the
                // parent reaps us.
                unsafe { pump_for(Duration::from_secs(20)) };
            }
        }
    }

    #[test]
    fn mutf16_encodes_and_null_terminates() {
        assert_eq!(mutf16("TTSB"), vec![0x54, 0x54, 0x53, 0x42, 0x0000]);
    }

    #[test]
    fn ready_receiver_acknowledges_show_request() {
        let (class, name, _guard, rx) = spawn_receiver("ready", 0);
        assert!(
            wait_for_marker(&rx, "__READY__", Duration::from_secs(10)),
            "receiver did not become ready"
        );

        let timing = DeliveryTiming {
            deadline: Duration::from_secs(5),
            discovery_interval: Duration::from_millis(20),
        };
        let started = Instant::now();
        let result = deliver_show_request(&class, &name, timing);
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Ok(())),
            "expected acknowledged delivery, got {result:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "delivery took too long: {elapsed:?}"
        );
    }

    #[test]
    fn delayed_receiver_is_discovered_after_retry() {
        // The window appears only after the child's startup delay, i.e. after
        // the sender's first lookup; the retry loop must still deliver.
        let (class, name, _guard, _rx) = spawn_receiver("ready", 500);

        let timing = DeliveryTiming {
            deadline: Duration::from_secs(8),
            discovery_interval: Duration::from_millis(20),
        };
        let started = Instant::now();
        let result = deliver_show_request(&class, &name, timing);
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Ok(())),
            "expected delayed receiver to be discovered and acknowledged, got {result:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(300),
            "delivery returned before the receiver could appear: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(8),
            "delivery exceeded the deadline: {elapsed:?}"
        );
    }

    #[test]
    fn request_before_callback_registration_is_processed_after() {
        let (class, name, _guard, rx) = spawn_receiver("pending", 0);
        assert!(wait_for_marker(&rx, "__READY__", Duration::from_secs(10)));

        let timing = DeliveryTiming {
            deadline: Duration::from_secs(5),
            discovery_interval: Duration::from_millis(20),
        };
        let result = deliver_show_request(&class, &name, timing);
        assert!(
            matches!(result, Ok(())),
            "expected acknowledged delivery, got {result:?}"
        );

        // The child processes the pending request only after registering the
        // callback; wait for its confirmation.
        assert!(
            wait_for_marker(&rx, "__CALLBACK__", Duration::from_secs(10)),
            "pending request was not processed after callback registration"
        );
    }

    #[test]
    fn unresponsive_receiver_times_out_within_budget() {
        let (class, name, _guard, rx) = spawn_receiver("unresponsive", 0);
        assert!(wait_for_marker(&rx, "__READY__", Duration::from_secs(10)));

        let timing = DeliveryTiming {
            deadline: Duration::from_secs(2),
            discovery_interval: Duration::from_millis(20),
        };
        let started = Instant::now();
        let result = deliver_show_request(&class, &name, timing);
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Err(DeliveryError::ReceiverUnresponsive)),
            "expected bounded timeout, got {result:?}"
        );
        assert!(
            elapsed < Duration::from_secs(6),
            "delivery did not respect the timeout budget: {elapsed:?}"
        );
    }

    #[test]
    fn no_receiver_fails_within_budget() {
        let (class, name) = unique_identity();

        let timing = DeliveryTiming {
            deadline: Duration::from_secs(1),
            discovery_interval: Duration::from_millis(20),
        };
        let started = Instant::now();
        let result = deliver_show_request(&class, &name, timing);
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Err(DeliveryError::ReceiverNotFound)),
            "expected receiver-not-found, got {result:?}"
        );
        assert!(
            elapsed < Duration::from_secs(4),
            "discovery exceeded the deadline budget: {elapsed:?}"
        );
    }
}
