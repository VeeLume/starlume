//! What Starlume holds in memory, for Settings → Performance.
//!
//! Commit size, not working set (CLAUDE.md, "Measure commit size"): what the
//! app has claimed from the system, across this process and the WebView2
//! processes it started. Inside the app, Rust allocates from the process
//! heap, which keeps small blocks it was given back: "held" is that memory,
//! free to the app but not returned to Windows.

use serde::Serialize;
use specta::Type;

use crate::error::AppError;

/// One sample, in megabytes.
#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryView {
    /// This process.
    pub app_mb: u32,
    /// The WebView2 processes under it.
    pub webview_mb: u32,
    pub webview_processes: u32,
    /// Of the app's heap: in use.
    pub heap_used_mb: u32,
    /// Of the app's heap: freed but not returned to Windows.
    pub heap_held_mb: u32,
}

/// Read fresh on every call: the page polls it only while it is open.
#[tauri::command]
#[specta::specta]
pub(crate) async fn performance_memory() -> Result<MemoryView, AppError> {
    tauri::async_runtime::spawn_blocking(sample)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[cfg(not(windows))]
fn sample() -> MemoryView {
    MemoryView::default()
}

#[cfg(windows)]
fn sample() -> MemoryView {
    use std::collections::HashMap;

    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }

    let own = std::process::id();
    let mut under = children
        .get(&Pid::from_u32(own))
        .cloned()
        .unwrap_or_default();
    let (mut webview, mut processes) = (0, 0);
    while let Some(pid) = under.pop() {
        if let Some(bytes) = win::private_bytes(pid.as_u32()) {
            webview += bytes;
            processes += 1;
        }
        if let Some(next) = children.get(&pid) {
            under.extend_from_slice(next);
        }
    }

    let (used, committed) = win::heap_used_and_committed();
    MemoryView {
        app_mb: megabytes(win::private_bytes(own).unwrap_or(0)),
        webview_mb: megabytes(webview),
        webview_processes: processes,
        heap_used_mb: megabytes(used),
        heap_held_mb: megabytes(committed.saturating_sub(used)),
    }
}

#[cfg(windows)]
fn megabytes(bytes: usize) -> u32 {
    (bytes / 1_000_000) as u32
}

/// The two system calls `sysinfo` doesn't offer: it reports working set.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    /// `PROCESS_MEMORY_COUNTERS_EX`.
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    /// `HEAP_SUMMARY`.
    #[repr(C)]
    #[derive(Default)]
    struct HeapSummaryOut {
        cb: u32,
        allocated: usize,
        committed: usize,
        reserved: usize,
        max_reserve: usize,
    }

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut MemoryCounters,
            size: u32,
        ) -> i32;
        fn GetProcessHeap() -> *mut c_void;
        fn HeapSummary(heap: *mut c_void, flags: u32, summary: *mut HeapSummaryOut) -> i32;
    }

    /// A process's commit size; `None` once it has gone.
    pub fn private_bytes(pid: u32) -> Option<usize> {
        // SAFETY: the handle is checked and closed here, and `counters` is
        // the size the call is told.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut counters = MemoryCounters {
                cb: size_of::<MemoryCounters>() as u32,
                ..MemoryCounters::default()
            };
            let ok = K32GetProcessMemoryInfo(process, &mut counters, counters.cb);
            CloseHandle(process);
            (ok != 0).then_some(counters.private_usage)
        }
    }

    /// The process heap's bytes in use and committed.
    pub fn heap_used_and_committed() -> (usize, usize) {
        let mut summary = HeapSummaryOut {
            cb: size_of::<HeapSummaryOut>() as u32,
            ..HeapSummaryOut::default()
        };
        // SAFETY: the process heap always exists, and `summary` says its size.
        let ok = unsafe { HeapSummary(GetProcessHeap(), 0, &mut summary) };
        if ok != 0 {
            (summary.allocated, summary.committed)
        } else {
            (0, 0)
        }
    }
}
