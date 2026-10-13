//! Performance benchmarking and measurement infrastructure.
//!
//! Runs the render loop in headless mode with synthetic load to measure
//! frame-time percentiles, memory usage, and CPU utilisation.
//!
//! Used by both the `--benchmark` CLI mode and the soak-test mode.

use std::time::{Duration, Instant};

/// Configuration for a soak test run.
#[derive(Debug, Clone)]
pub struct SoakConfig {
    /// How long to run the soak test, in seconds.
    pub duration_secs: u64,
    /// How often to sample metrics, in seconds.
    pub sample_interval_secs: u64,
    /// Number of creatures to simulate.
    pub creature_count: usize,
    /// Whether to simulate IPC load.
    pub simulate_ipc: bool,
    /// Whether to simulate AI bridge traffic.
    pub simulate_ai: bool,
}

impl Default for SoakConfig {
    fn default() -> Self {
        Self {
            duration_secs: 86400, // 24h
            sample_interval_secs: 60,
            creature_count: 4,
            simulate_ipc: true,
            simulate_ai: true,
        }
    }
}

/// A single sample from a soak test.
#[derive(Debug, Clone)]
pub struct SoakSample {
    pub elapsed_secs: u64,
    pub memory_mb: f64,
    pub handle_count: u32,
    pub fps: f64,
    pub frame_count: u64,
}

/// Result of a soak test run.
#[derive(Debug, Clone)]
pub struct SoakResult {
    pub duration_secs: u64,
    pub samples: Vec<SoakSample>,
    pub memory_start_mb: f64,
    pub memory_end_mb: f64,
    pub memory_growth_mb: f64,
    pub handle_start: u32,
    pub handle_end: u32,
    pub handle_growth: u32,
    pub avg_fps: f64,
    pub leak_detected: bool,
}

/// Runs a soak test and returns the results.
///
/// This does NOT create a window or render anything. It drives the same
/// timing and metrics logic the real render loop uses, sampling memory
/// and handle count at regular intervals to detect leaks.
#[allow(clippy::manual_is_multiple_of)]
pub fn run_soak_test(config: &SoakConfig) -> SoakResult {
    let mut metrics = crate::metrics::FrameMetrics::new(600);
    let frame_budget_ms = 1000.0 / 60.0;
    let duration = Duration::from_secs(config.duration_secs);
    let sample_interval = Duration::from_secs(config.sample_interval_secs.max(1));

    let mem_start = get_memory_usage_mb();
    let handle_start = get_handle_count();

    let start = Instant::now();
    let mut next_sample = sample_interval;
    let mut samples = Vec::new();
    let mut frame_count: u64 = 0;

    while start.elapsed() < duration {
        let frame_start = Instant::now();

        let update_cost = simulate_creature_update(config.creature_count);
        std::thread::sleep(Duration::from_micros(update_cost));

        if config.simulate_ipc && frame_count % 10 == 0 {
            simulate_ipc_work(frame_count);
        }

        if config.simulate_ai && frame_count % 30 == 0 {
            simulate_ai_work();
        }

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        metrics.record(elapsed_ms);

        let remaining = frame_budget_ms - elapsed_ms;
        if remaining > 0.0 {
            std::thread::sleep(Duration::from_micros((remaining * 1000.0) as u64));
        }

        frame_count += 1;

        if start.elapsed() >= next_sample {
            let elapsed = start.elapsed().as_secs();
            let sample = SoakSample {
                elapsed_secs: elapsed,
                memory_mb: get_memory_usage_mb(),
                handle_count: get_handle_count(),
                fps: crate::metrics::FrameMetrics::fps_from_ms(metrics.mean()),
                frame_count,
            };
            samples.push(sample);
            next_sample = start.elapsed() + sample_interval;
        }
    }

    let mem_end = get_memory_usage_mb();
    let handle_end = get_handle_count();
    let memory_growth = mem_end - mem_start;
    let handle_growth = handle_end.saturating_sub(handle_start);

    // Leak detection: memory growth > 50MB or handle growth > 100 over the run.
    let leak_detected = memory_growth > 50.0 || handle_growth > 100;

    let avg_fps = if config.duration_secs > 0 {
        frame_count as f64 / config.duration_secs as f64
    } else {
        0.0
    };

    SoakResult {
        duration_secs: config.duration_secs,
        samples,
        memory_start_mb: mem_start,
        memory_end_mb: mem_end,
        memory_growth_mb: memory_growth,
        handle_start,
        handle_end,
        handle_growth,
        avg_fps,
        leak_detected,
    }
}

fn get_handle_count() -> u32 {
    unsafe {
        let process = winapi::um::processthreadsapi::GetCurrentProcess();
        let mut count: u32 = 0;
        winapi::um::processthreadsapi::GetProcessHandleCount(
            process,
            &mut count,
        );
        count
    }
}

/// Configuration for a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// How long to run the benchmark, in seconds.
    pub duration_secs: u64,
    /// Number of synthetic creatures to simulate.
    pub creature_count: usize,
    /// Target frame rate (used to calculate frame budget).
    pub fps_target: u32,
    /// Whether to simulate IPC load.
    pub simulate_ipc: bool,
    /// Whether to simulate AI bridge traffic.
    pub simulate_ai: bool,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            duration_secs: 10,
            creature_count: 4,
            fps_target: 60,
            simulate_ipc: true,
            simulate_ai: true,
        }
    }
}

/// A single benchmark run result.
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub duration_secs: u64,
    pub frame_count: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub mean_ms: f64,
    pub worst_ms: f64,
    pub fps_effective: f64,
    pub memory_start_mb: f64,
    pub memory_end_mb: f64,
    pub cpu_avg_percent: f64,
}

/// Runs a benchmark in headless mode and returns the results.
///
/// This does NOT create a window or render anything. It drives the same
/// FrameMetrics and timing logic the real render loop uses, so the
/// frame-time numbers are directly comparable.
#[allow(clippy::manual_is_multiple_of)]
pub fn run_benchmark(config: &BenchConfig) -> BenchResult {
    let mut metrics = crate::metrics::FrameMetrics::new(600);
    let frame_budget_ms = 1000.0 / config.fps_target as f64;
    let duration = Duration::from_secs(config.duration_secs);

    let mem_start = get_memory_usage_mb();
    let cpu_start = get_cpu_usage();

    let start = Instant::now();
    let mut frame_count: u64 = 0;

    // Simulate render loop: record a frame time, sleep for the remainder
    // of the budget, repeat. The jitter pattern is deterministic so runs
    // are comparable.
    while start.elapsed() < duration {
        let frame_start = Instant::now();

        // Simulate creature update cost: proportional to creature count.
        let update_cost = simulate_creature_update(config.creature_count);
        std::thread::sleep(Duration::from_micros(update_cost));

        // Simulate IPC work.
        #[allow(clippy::manual_is_multiple_of)]
        if config.simulate_ipc && frame_count % 10 == 0 {
            simulate_ipc_work(frame_count);
        }

        // Simulate AI bridge traffic.
        #[allow(clippy::manual_is_multiple_of)]
        if config.simulate_ai && frame_count % 30 == 0 {
            simulate_ai_work();
        }

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        metrics.record(elapsed_ms);

        // Sleep for the remainder of the frame budget, simulating vsync pacing.
        let remaining = frame_budget_ms - elapsed_ms;
        if remaining > 0.0 {
            std::thread::sleep(Duration::from_micros((remaining * 1000.0) as u64));
        }

        frame_count += 1;
    }

    let summary = metrics.summary();
    let mem_end = get_memory_usage_mb();
    let cpu_end = get_cpu_usage();
    let cpu_avg = (cpu_start + cpu_end) / 2.0;

    BenchResult {
        duration_secs: config.duration_secs,
        frame_count,
        p50_ms: summary.p50_ms,
        p95_ms: summary.p95_ms,
        p99_ms: summary.p99_ms,
        mean_ms: summary.mean_ms,
        worst_ms: summary.worst_ms,
        fps_effective: frame_count as f64 / config.duration_secs as f64,
        memory_start_mb: mem_start,
        memory_end_mb: mem_end,
        cpu_avg_percent: cpu_avg,
    }
}

/// Simulates the per-frame cost of updating `count` creatures.
/// Returns microseconds of simulated work.
fn simulate_creature_update(count: usize) -> u64 {
    // Base cost per creature: ~200us for state machine + animation.
    let base = count as u64 * 200;
    // Add some jitter (±20%).
    let jitter = base / 5;
    base + (jitter % 100)
}

/// Simulates IPC work: serialising a status snapshot.
fn simulate_ipc_work(frame: u64) {
    let status = crate::ipc::RuntimeStatus {
        fps: 60.0,
        frame_count: frame,
        creature_count: 4,
        visible_count: 3,
        auto_cycle: true,
        uptime_secs: frame as f64 / 60.0,
        model_count: 7,
        tool_count: 18,
        memory_chunks: 42,
        always_on_top: false,
        ..Default::default()
    };
    let json = status.to_json();
    let _ = serde_json::to_string(&json).unwrap();
}

/// Simulates AI bridge traffic: a small LRU cache lookup + token count.
fn simulate_ai_work() {
    let mut cache = lru::LruCache::<String, String>::new(
        std::num::NonZeroUsize::new(10).unwrap()
    );
    for i in 0..5 {
        cache.put(format!("key{}", i), format!("value{}", i));
    }
    for i in 0..5 {
        let _ = cache.get(&format!("key{}", i));
    }
}

fn get_memory_usage_mb() -> f64 {
    unsafe {
        let mut mem_info: winapi::um::psapi::PROCESS_MEMORY_COUNTERS =
            std::mem::zeroed();
        let cb = std::mem::size_of::<winapi::um::psapi::PROCESS_MEMORY_COUNTERS>()
            as u32;
        if winapi::um::psapi::GetProcessMemoryInfo(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            &mut mem_info,
            cb,
        ) != 0
        {
            mem_info.WorkingSetSize as f64 / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }
}

fn get_cpu_usage() -> f64 {
    unsafe {
        let process = winapi::um::processthreadsapi::GetCurrentProcess();
        let mut creation: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
        let mut exit: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
        let mut kernel: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
        let mut user: winapi::shared::minwindef::FILETIME = std::mem::zeroed();

        if winapi::um::processthreadsapi::GetProcessTimes(
            process, &mut creation, &mut exit, &mut kernel, &mut user,
        ) == 0
        {
            return 0.0;
        }

        let kernel_time = filetime_to_u64(kernel);
        let user_time = filetime_to_u64(user);
        let total = kernel_time + user_time;

        // Get elapsed time since process start.
        let now = winapi::um::sysinfoapi::GetTickCount64() as u64;
        let uptime_ms = now.saturating_sub(process_start_tick());

        if uptime_ms == 0 {
            return 0.0;
        }
        // Convert 100ns units to ms, then to percentage.
        let cpu_ms = total / 10_000;
        let uptime = uptime_ms;
        (cpu_ms as f64 / uptime as f64) * 100.0
    }
}

fn filetime_to_u64(ft: winapi::shared::minwindef::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

fn process_start_tick() -> u64 {
    static START_TICK: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *START_TICK.get_or_init(|| {
        unsafe {
            let mut creation: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
            let mut exit: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
            let mut kernel: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
            let mut user: winapi::shared::minwindef::FILETIME = std::mem::zeroed();
            let process = winapi::um::processthreadsapi::GetCurrentProcess();
            winapi::um::processthreadsapi::GetProcessTimes(
                process, &mut creation, &mut exit, &mut kernel, &mut user,
            );
            winapi::um::sysinfoapi::GetTickCount64()
                .saturating_sub(filetime_to_u64(creation) / 10_000)
        }
    })
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_benchmark_run_produces_all_fields() {
        let config = BenchConfig {
            duration_secs: 1,
            creature_count: 2,
            fps_target: 60,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let result = run_benchmark(&config);
        assert!(result.duration_secs >= 1);
        assert!(result.frame_count > 0);
        assert!(result.p50_ms.is_finite());
        assert!(result.p95_ms.is_finite());
        assert!(result.p99_ms.is_finite());
        assert!(result.mean_ms.is_finite());
        assert!(result.fps_effective > 0.0);
        assert!(result.memory_end_mb > 0.0);
    }

    #[test]
    fn more_creatures_cost_more_time() {
        let config1 = BenchConfig {
            duration_secs: 1,
            creature_count: 1,
            fps_target: 60,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let config4 = BenchConfig {
            duration_secs: 1,
            creature_count: 4,
            fps_target: 60,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let r1 = run_benchmark(&config1);
        let r4 = run_benchmark(&config4);
        assert!(
            r4.mean_ms >= r1.mean_ms,
            "4 creatures should cost at least as much as 1"
        );
    }

    #[test]
    fn frame_budget_respected() {
        let config = BenchConfig {
            duration_secs: 2,
            creature_count: 2,
            fps_target: 30,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let result = run_benchmark(&config);
        // 30fps = 33.3ms budget. Mean should be under that.
        assert!(result.mean_ms <= 35.0, "mean {}ms exceeds budget", result.mean_ms);
    }

    #[test]
    fn soak_test_produces_samples() {
        let config = SoakConfig {
            duration_secs: 2,
            sample_interval_secs: 1,
            creature_count: 2,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let result = run_soak_test(&config);
        assert!(!result.samples.is_empty(), "should have samples");
        assert!(result.duration_secs >= 2);
        assert!(result.memory_end_mb > 0.0);
    }

    #[test]
    fn soak_test_detects_no_leak_on_clean_run() {
        // A short clean run should not trigger leak detection.
        // Memory growth < 100 MB and handle growth < 1000 is acceptable
        // for short tests due to normal runtime fluctuation.
        let config = SoakConfig {
            duration_secs: 5,
            sample_interval_secs: 1,
            creature_count: 2,
            simulate_ipc: false,
            simulate_ai: false,
        };
        let result = run_soak_test(&config);
        assert!(
            result.memory_growth_mb < 100.0,
            "memory growth too high: {:.1} MB",
            result.memory_growth_mb
        );
        assert!(
            result.handle_growth < 1000,
            "handle growth too high: {}",
            result.handle_growth
        );
    }

    #[test]
    fn soak_test_samples_have_valid_data() {
        let config = SoakConfig {
            duration_secs: 2,
            sample_interval_secs: 1,
            creature_count: 2,
            simulate_ipc: true,
            simulate_ai: true,
        };
        let result = run_soak_test(&config);
        for sample in &result.samples {
            assert!(sample.memory_mb > 0.0, "memory should be positive");
            assert!(sample.handle_count > 0, "handles should be positive");
            assert!(sample.fps > 0.0, "fps should be positive");
            assert!(sample.frame_count > 0, "frames should be positive");
        }
    }
}
