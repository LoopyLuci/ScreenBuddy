//! Frame-time metrics.
//!
//! `RuntimeStatus` reported an instantaneous FPS and a frame count, which says
//! nothing about stutter: a run can average 60fps and still hitch badly, and
//! that is the thing a user actually notices. This keeps a rolling window of
//! frame times so percentiles can be reported.
//!
//! A fixed-capacity ring buffer is used rather than a growing vector so the
//! memory cost is constant however long the app runs.

use std::collections::VecDeque;

/// A rolling window of frame times in milliseconds.
#[derive(Debug, Clone)]
pub struct FrameMetrics {
    samples: VecDeque<f64>,
    capacity: usize,
    /// Every frame ever recorded, including those evicted from the window.
    total_frames: u64,
    /// Worst frame time seen since the last reset, not just in the window: a
    /// single bad hitch is exactly what a user reports and it must not age out.
    worst_ms: f64,
}

/// A point-in-time summary, cheap to clone into a status snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetricsSummary {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub mean_ms: f64,
    pub worst_ms: f64,
    /// Frames in the current window.
    pub window_frames: usize,
    pub total_frames: u64,
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self::new(600)
    }
}

impl FrameMetrics {
    /// `capacity` is how many recent frames to keep; 600 is ten seconds at 60fps.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
            total_frames: 0,
            worst_ms: 0.0,
        }
    }

    /// Record one frame time. Non-finite and negative values are ignored: a bad
    /// sample must not poison every percentile.
    pub fn record(&mut self, ms: f64) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
        self.total_frames += 1;
        if ms > self.worst_ms {
            self.worst_ms = ms;
        }
    }

    /// Nearest-rank percentile over the window, in milliseconds.
    ///
    /// Returns 0 for an empty window rather than NaN, so a caller can serialise
    /// it without special-casing.
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p = p.clamp(0.0, 1.0);
        // Nearest-rank: the index of the smallest value at or above the p-th
        // percentile.
        let rank = (p * sorted.len() as f64).ceil() as usize;
        let idx = rank.saturating_sub(1).min(sorted.len() - 1);
        sorted[idx]
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn summary(&self) -> MetricsSummary {
        MetricsSummary {
            p50_ms: self.percentile(0.50),
            p95_ms: self.percentile(0.95),
            p99_ms: self.percentile(0.99),
            mean_ms: self.mean(),
            worst_ms: self.worst_ms,
            window_frames: self.samples.len(),
            total_frames: self.total_frames,
        }
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.total_frames = 0;
        self.worst_ms = 0.0;
    }

    /// Effective frames per second for a frame time.
    pub fn fps_from_ms(ms: f64) -> f64 {
        if ms <= 0.0 {
            0.0
        } else {
            1000.0 / ms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_window_summarises_as_zero_not_nan() {
        let m = FrameMetrics::new(10);
        let s = m.summary();
        assert_eq!(s.p50_ms, 0.0);
        assert_eq!(s.p95_ms, 0.0);
        assert_eq!(s.mean_ms, 0.0);
        assert_eq!(s.window_frames, 0);
        assert!(s.p50_ms.is_finite());
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let mut m = FrameMetrics::new(100);
        for i in 1..=100 {
            m.record(i as f64);
        }
        // 100 samples of 1..=100: p50 is 50, p95 is 95, p99 is 99.
        assert_eq!(m.percentile(0.50), 50.0);
        assert_eq!(m.percentile(0.95), 95.0);
        assert_eq!(m.percentile(0.99), 99.0);
    }

    #[test]
    fn a_single_hitch_shows_up_in_p99_and_worst() {
        let mut m = FrameMetrics::new(100);
        for _ in 0..99 {
            m.record(16.0);
        }
        m.record(250.0); // one bad frame
        let s = m.summary();
        assert_eq!(s.p50_ms, 16.0, "the median is unaffected by one hitch");
        assert_eq!(s.worst_ms, 250.0);
        assert!(s.p99_ms >= 16.0);
    }

    #[test]
    fn the_worst_frame_is_remembered_after_it_leaves_the_window() {
        let mut m = FrameMetrics::new(5);
        m.record(500.0);
        for _ in 0..50 {
            m.record(16.0);
        }
        // The hitch has aged out of the window but must still be reported: it is
        // the thing a user would complain about.
        assert_eq!(m.summary().window_frames, 5);
        assert_eq!(m.summary().worst_ms, 500.0);
    }

    #[test]
    fn the_window_does_not_grow_past_its_capacity() {
        let mut m = FrameMetrics::new(10);
        for i in 0..1000 {
            m.record(i as f64);
        }
        assert_eq!(m.summary().window_frames, 10);
        assert_eq!(m.summary().total_frames, 1000);
    }

    #[test]
    fn the_mean_covers_the_window_only() {
        let mut m = FrameMetrics::new(3);
        m.record(100.0);
        m.record(10.0);
        m.record(10.0);
        m.record(10.0); // evicts the 100
        assert!((m.mean() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn non_finite_and_negative_samples_are_ignored() {
        let mut m = FrameMetrics::new(10);
        m.record(f64::NAN);
        m.record(f64::INFINITY);
        m.record(-5.0);
        assert_eq!(m.summary().window_frames, 0);
        assert_eq!(m.summary().total_frames, 0);
    }

    #[test]
    fn a_zero_capacity_is_clamped_rather_than_panicking() {
        let mut m = FrameMetrics::new(0);
        m.record(16.0);
        assert_eq!(m.summary().window_frames, 1);
    }

    #[test]
    fn reset_clears_everything() {
        let mut m = FrameMetrics::new(10);
        m.record(500.0);
        m.reset();
        let s = m.summary();
        assert_eq!(s.total_frames, 0);
        assert_eq!(s.worst_ms, 0.0);
    }

    #[test]
    fn fps_from_ms_handles_zero_without_dividing_by_zero() {
        assert_eq!(FrameMetrics::fps_from_ms(0.0), 0.0);
        assert_eq!(FrameMetrics::fps_from_ms(-1.0), 0.0);
        assert!((FrameMetrics::fps_from_ms(16.0) - 62.5).abs() < 1e-9);
    }
}
