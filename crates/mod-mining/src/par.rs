//! Heavy solves run on a thread pool built for the job and dropped when it
//! ends: a resident tray app keeps no idle threads. While the job runs its
//! threads sit below normal priority, because the game is likely running
//! beside it and must keep its frames.

/// Run `f` on a job pool, one thread per core. Inside a job already (a
/// solve that grades rocks, each grade simulating many tries), `f` joins
/// the running job's pool instead, so nested work shares one set of threads.
pub fn job<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    if rayon::current_thread_index().is_some() {
        return f();
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    match rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("mining-job-{i}"))
        .start_handler(|_| below_normal_priority())
        .build()
    {
        Ok(pool) => pool.install(f),
        // No pool (out of threads): the work still gets done, here.
        Err(_) => f(),
    }
}

#[cfg(windows)]
fn below_normal_priority() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: GetCurrentThread is a pseudo-handle to this thread, valid for
    // the call; a failed priority change leaves the default, which is fine.
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(windows))]
fn below_normal_priority() {}

#[cfg(test)]
mod tests {
    use rayon::prelude::*;

    #[test]
    fn nested_jobs_share_the_outer_pool_and_keep_order() {
        let out: Vec<Vec<u32>> = super::job(|| {
            (0..4u32)
                .into_par_iter()
                .map(|i| super::job(|| (0..8u32).into_par_iter().map(|k| i * 8 + k).collect()))
                .collect()
        });
        let flat: Vec<u32> = out.into_iter().flatten().collect();
        assert_eq!(flat, (0..32).collect::<Vec<_>>());
    }
}
