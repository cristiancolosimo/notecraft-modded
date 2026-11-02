//! The performance counter: a small readout at the top right of the window with the frame rate,
//! the time frames take, the process's processor and memory use, and how big the page is. It is
//! off unless the owner turns it on (File, Options), and it costs next to nothing while off.
//!
//! The process figures come from `/proc` (Linux and Android); elsewhere they are left out.

use egui::{Color32, Context, FontId, Id, Order, Pos2, Rect, vec2};
use std::collections::VecDeque;
use web_time::{Duration, Instant};

/// How long the frame rate is averaged over.
const WINDOW: Duration = Duration::from_secs(1);
/// A gap this long between two frames is the app being idle, not a slow frame.
const IDLE_GAP: f32 = 0.25;
/// How often the text is rebuilt, so it can be read.
const REFRESH: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct Perf {
    frames: VecDeque<(Instant, f32)>,
    last: Option<Instant>,
    sampler: ProcessSampler,
    text: Vec<String>,
    built: Option<Instant>,
}

impl Perf {
    /// Called once for every frame the window draws.
    pub fn frame(&mut self, now: Instant) {
        let gap = self.last.map(|t| now.duration_since(t).as_secs_f32());
        self.last = Some(now);
        // A frame after a long idle is not slow: it is the first after a rest.
        let ms = gap.filter(|g| *g < IDLE_GAP).map_or(0.0, |g| g * 1000.0);
        self.frames.push_back((now, ms));
        while self.frames.front().is_some_and(|(t, _)| now.duration_since(*t) > WINDOW) {
            self.frames.pop_front();
        }
    }

    /// Frames drawn in the last second.
    pub fn fps(&self) -> usize {
        self.frames.len()
    }

    /// The mean and the worst time between frames over the last second, in milliseconds, leaving
    /// out frames that followed a rest.
    pub fn frame_ms(&self) -> (f32, f32) {
        let times: Vec<f32> = self.frames.iter().map(|(_, ms)| *ms).filter(|ms| *ms > 0.0).collect();
        if times.is_empty() {
            return (0.0, 0.0);
        }
        (times.iter().sum::<f32>() / times.len() as f32, times.iter().copied().fold(0.0, f32::max))
    }

    /// Draws the readout over `area`'s top right corner. `page` describes the page, e.g. its ink.
    pub fn show(&mut self, ctx: &Context, area: Rect, page: &str) {
        let now = Instant::now();
        if self.built.is_none_or(|t| now.duration_since(t) >= REFRESH) {
            self.built = Some(now);
            self.text = self.lines(ctx, page);
        }
        // Rebuilt twice a second even when nothing else draws.
        ctx.request_repaint_after(REFRESH);
        let font = FontId::monospace(12.0);
        let galleys: Vec<_> = self.text.iter().map(|l| ctx.fonts_mut(|f| f.layout_no_wrap(l.clone(), font.clone(), Color32::WHITE))).collect();
        let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 14.0;
        let height = galleys.iter().map(|g| g.size().y).sum::<f32>() + 10.0;
        let rect = Rect::from_min_size(Pos2::new(area.max.x - width - 8.0, area.min.y + 6.0), vec2(width, height));
        // Over everything and out of the way: it never takes a touch.
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("perf-counter")));
        painter.rect_filled(rect, 6.0, Color32::from_black_alpha(190));
        let mut y = rect.min.y + 5.0;
        for galley in galleys {
            let h = galley.size().y;
            painter.galley(Pos2::new(rect.min.x + 7.0, y), galley, Color32::from_rgb(0xb8, 0xf5, 0xc0));
            y += h;
        }
    }

    fn lines(&mut self, ctx: &Context, page: &str) -> Vec<String> {
        let (mean, worst) = self.frame_ms();
        let mut lines = vec![format!("{:>3} fps  {:>5.1} ms (worst {:.0})", self.fps(), mean, worst)];
        if let Some(p) = self.sampler.sample() {
            lines.push(format!("cpu {:>3.0}%  mem {:>4} MB  threads {}", p.cpu_percent, p.rss_kb / 1024, p.threads));
        }
        let size = ctx.content_rect().size();
        lines.push(format!("{:.0}x{:.0} pt @ {:.2}x", size.x, size.y, ctx.pixels_per_point()));
        if !page.is_empty() {
            lines.push(page.to_string());
        }
        lines
    }
}

/// The frame rates the owner can cap the window at, in frames a second. 0 is no cap.
pub const FPS_LIMITS: [u32; 6] = [0, 30, 45, 60, 90, 120];

/// How long to wait before drawing the next frame so frames are no closer together than
/// `limit` allows: nothing when there is no cap, or when enough time has gone by already.
pub fn wait_for(since_last_frame: Duration, limit: u32) -> Duration {
    if limit == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f32(1.0 / limit as f32).saturating_sub(since_last_frame)
}

/// What the process is using.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Process {
    pub cpu_percent: f32,
    pub rss_kb: u64,
    pub threads: u32,
}

#[derive(Default)]
struct ProcessSampler {
    previous: Option<(Instant, u64)>,
}

/// Clock ticks a second in `/proc/self/stat`. It is 100 on Linux and Android.
const TICKS_PER_SECOND: f64 = 100.0;

impl ProcessSampler {
    fn sample(&mut self) -> Option<Process> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let (rss_kb, threads) = parse_status(&status)?;
        let ticks = parse_stat_ticks(&stat)?;
        let now = Instant::now();
        let cpu_percent = match self.previous {
            Some((then, before)) => {
                let wall = now.duration_since(then).as_secs_f64().max(0.001);
                (ticks.saturating_sub(before) as f64 / TICKS_PER_SECOND / wall * 100.0) as f32
            }
            None => 0.0,
        };
        self.previous = Some((now, ticks));
        Some(Process { cpu_percent, rss_kb, threads })
    }
}

/// `VmRSS` in kilobytes and `Threads` from `/proc/self/status`.
pub fn parse_status(text: &str) -> Option<(u64, u32)> {
    let field =
        |name: &str| text.lines().find_map(|l| l.strip_prefix(name)).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse::<u64>().ok());
    Some((field("VmRSS:")?, u32::try_from(field("Threads:")?).ok()?))
}

/// Processor time the process has used (user plus system, in clock ticks) from `/proc/self/stat`.
/// The command name in the second field may hold spaces and brackets, so it is skipped by its closing bracket.
pub fn parse_stat_ticks(text: &str) -> Option<u64> {
    let after_name = text.rsplit_once(')')?.1;
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    // After the name: state is field 0, utime is field 11 and stime field 12.
    Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_cap_waits_only_as_long_as_it_must() {
        let ms = Duration::from_millis;
        assert_eq!(wait_for(ms(1), 0), Duration::ZERO, "no cap, no wait");
        assert!((wait_for(ms(5), 30).as_secs_f32() - (1.0 / 30.0 - 0.005)).abs() < 0.001, "30 a second leaves a 33 ms frame");
        assert_eq!(wait_for(ms(40), 30), Duration::ZERO, "a frame that was already slow is not delayed");
        assert!(wait_for(ms(0), 120) < ms(9) && wait_for(ms(0), 120) > ms(8));
        assert_eq!(FPS_LIMITS, [0, 30, 45, 60, 90, 120]);
    }

    #[test]
    fn the_status_file_gives_memory_and_threads() {
        let text = "Name:\tnotecraft\nVmPeak:\t  900000 kB\nVmRSS:\t  312344 kB\nThreads:\t21\n";
        assert_eq!(parse_status(text), Some((312344, 21)));
        assert_eq!(parse_status("nothing useful"), None);
    }

    #[test]
    fn the_stat_line_gives_processor_time_even_with_a_awkward_name() {
        // pid (name) state ppid pgrp session tty tpgid flags minflt cminflt majflt cmajflt utime stime ...
        let line = "4242 (my (odd) app) S 1 4242 4242 0 -1 4194560 100 0 0 0 1500 250 0 0 20 0 21 0 12345 1000000 5000";
        assert_eq!(parse_stat_ticks(line), Some(1750));
        assert_eq!(parse_stat_ticks("garbage"), None);
    }

    #[test]
    fn frames_are_counted_over_a_second_and_a_rest_is_not_a_slow_frame() {
        let mut perf = Perf::default();
        let start = Instant::now();
        for i in 0..30 {
            perf.frame(start + Duration::from_millis(i * 20));
        }
        assert_eq!(perf.fps(), 30, "thirty frames inside one second");
        let (mean, worst) = perf.frame_ms();
        assert!((mean - 20.0).abs() < 1.0 && (worst - 20.0).abs() < 1.0, "{mean} {worst}");

        // Two seconds later the first frame after the rest is not counted as a 2000 ms frame.
        perf.frame(start + Duration::from_millis(2600));
        assert_eq!(perf.fps(), 1);
        assert_eq!(perf.frame_ms(), (0.0, 0.0));
    }
}
