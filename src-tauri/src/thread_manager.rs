//! Thread Pool Manager
//!
//! Manages background threads with proper lifecycle and limits
#![allow(dead_code)] // Ready for integration when needed
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use parking_lot::Mutex;
use std::collections::HashMap;

/// Thread pool manager for controlled thread creation
pub struct ThreadPoolManager {
    threads: Mutex<HashMap<String, JoinHandle<()>>>,
    max_threads: usize,
}

impl ThreadPoolManager {
    /// Create a new thread pool manager with CPU-based thread limit
    pub fn new() -> Arc<Self> {
        let max_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Arc::new(Self {
            threads: Mutex::new(HashMap::new()),
            max_threads,
        })
    }

    /// Spawn a new named thread, replacing any existing thread with the same name
    pub fn spawn<F>(&self, name: String, f: F) -> Result<(), String>
    where
        F: FnOnce() + Send + 'static,
    {
        let mut threads = self.threads.lock();

        // Check limit
        if threads.len() >= self.max_threads {
            return Err(format!("Thread limit reached: {}", self.max_threads));
        }

        // Join previous thread with same name if exists
        if let Some(handle) = threads.remove(&name) {
            eprintln!("[THREAD_POOL] Joining previous thread: {}", name);
            let _ = handle.join();
        }

        let handle = thread::spawn(f);
        threads.insert(name, handle);

        Ok(())
    }

    /// Shutdown all threads gracefully
    pub fn shutdown(&self) {
        let mut threads = self.threads.lock();
        eprintln!("[THREAD_POOL] Shutting down {} threads", threads.len());
        for (name, handle) in threads.drain() {
            eprintln!("[THREAD_POOL] Joining thread: {}", name);
            let _ = handle.join();
        }
        eprintln!("[THREAD_POOL] All threads shut down");
    }

    /// Get current thread count
    pub fn thread_count(&self) -> usize {
        self.threads.lock().len()
    }
}

impl Default for ThreadPoolManager {
    fn default() -> Self {
        Self {
            threads: Mutex::new(HashMap::new()),
            max_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        }
    }
}
