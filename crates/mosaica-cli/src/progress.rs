//! A long-running job's progress on stderr, drawn from [`Progress`] snapshots.
//!
//! [`Renderer`] takes snapshots from any source and decides what to print for the chosen
//! [`Mode`]: a source that polls gives it each snapshot as a tick, and a job that runs in this
//! process also tells it when each step ends and begins. [`Feed`] and [`show`] are that source for
//! a job that counts into a [`Tracker`].

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mosaica_types::process::human_bytes;
use mosaica_types::progress::{JobState, Progress, Tracker, Unit};

/// How progress is shown on stderr, as `mosaica build --progress` describes each mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Mode {
    Auto,
    Bar,
    Plain,
    Json,
    None,
}

const PLAIN_EVERY: Duration = Duration::from_secs(30);
const JSON_EVERY: Duration = Duration::from_secs(1);
const BAR_EVERY: Duration = Duration::from_millis(250);
const BAR_CELLS: usize = 24;

/// Decides which snapshots to print and prints them.
pub(crate) struct Renderer {
    /// `Auto` decided against stderr.
    mode: Mode,
    /// The step of the last snapshot printed: its phase, name and context.
    shown: Option<(u32, String, Option<String>)>,
    printed: Option<Instant>,
    /// Whether the bar is on the screen, with the cursor at the start of its first line.
    drawn: bool,
}

impl Renderer {
    pub(crate) fn new(mode: Mode) -> Renderer {
        let mode = match mode {
            Mode::Auto if std::io::IsTerminal::is_terminal(&std::io::stderr()) => Mode::Bar,
            Mode::Auto => Mode::Plain,
            mode => mode,
        };
        Renderer {
            mode,
            shown: None,
            printed: None,
            drawn: false,
        }
    }

    /// How often a source should give this renderer a snapshot.
    pub(crate) fn interval(&self) -> Duration {
        match self.mode {
            Mode::Bar => BAR_EVERY,
            _ => JSON_EVERY,
        }
    }

    /// The last snapshot of a step that has ended, with its whole count.
    pub(crate) fn step_ended(&mut self, progress: &Progress) {
        if matches!(self.mode, Mode::Plain | Mode::Json) {
            self.print(progress);
        }
    }

    /// The first snapshot of a step that has begun.
    pub(crate) fn step_begun(&mut self, progress: &Progress) {
        match self.mode {
            Mode::Plain | Mode::Json => self.print(progress),
            _ => self.tick(progress),
        }
    }

    /// A snapshot taken while the job runs, printed when it shows a step the last print did not,
    /// or when the mode's interval has passed since the last print.
    pub(crate) fn tick(&mut self, progress: &Progress) {
        let since = self.printed.map(|at| at.elapsed());
        let due = match self.mode {
            Mode::Plain => since.is_none_or(|s| s >= PLAIN_EVERY),
            Mode::Json => since.is_none_or(|s| s >= JSON_EVERY),
            Mode::Bar => since.is_none_or(|s| s >= BAR_EVERY / 2),
            Mode::Auto | Mode::None => false,
        };
        if due || self.mode != Mode::Bar && self.shown != Some(step_of(progress)) {
            self.print(progress);
        }
    }

    /// Print the job's last snapshot, whatever the interval.
    pub(crate) fn finish(&mut self, progress: &Progress) {
        self.print(progress);
        if self.drawn {
            eprint!("\n\n");
            self.drawn = false;
        }
    }

    fn print(&mut self, progress: &Progress) {
        let mut err = std::io::stderr().lock();
        let _ = match self.mode {
            Mode::Plain => writeln!(
                err,
                "{} [{}/{} {}] {}",
                clock(SystemTime::now()),
                progress.phase.index,
                progress.phase.of,
                progress.phase.name,
                line(progress)
            ),
            Mode::Json => match serde_json::to_string(progress) {
                Ok(json) => writeln!(err, "{json}"),
                Err(_) => Ok(()),
            },
            Mode::Bar => {
                // The last draw left the cursor at the start of the bar's first line, so what the
                // job has written to stderr since then is written over the bar, and the bar is
                // cleared from the end of it and drawn below. Where the job's lines are shorter
                // than the bar's, the rest of the old bar's lines stays beside them. Lines do not
                // wrap while the bar is drawn, so moving up two lines reaches its first.
                self.drawn = true;
                write!(
                    err,
                    "\x1b[?7l\x1b[J[{}/{}] {}\n{}{}\n\x1b[2A\r\x1b[?7h",
                    progress.phase.index,
                    progress.phase.of,
                    progress.phase.name,
                    bar(progress),
                    line(progress)
                )
            }
            Mode::Auto | Mode::None => Ok(()),
        };
        let _ = err.flush();
        self.shown = Some(step_of(progress));
        self.printed = Some(Instant::now());
    }
}

/// The phase, name and context that tell one step from another.
fn step_of(progress: &Progress) -> (u32, String, Option<String>) {
    (
        progress.phase.index,
        progress.step.to_string(),
        progress.context.clone(),
    )
}

/// The step, its count, rate, time remaining, the job's elapsed time and resident memory, and
/// the job's state once it has ended.
fn line(progress: &Progress) -> String {
    let mut out = progress.step.to_string();
    if let Some(context) = &progress.context {
        out.push(' ');
        out.push_str(context);
    }
    if let (Some(done), Some(total), Some(unit)) = (progress.done, progress.total, progress.unit) {
        out.push_str(&format!(
            "  {}/{}{}",
            amount(done as f64, unit),
            amount(total as f64, unit),
            unit_name(unit)
        ));
        if let Some(rate) = progress.rate {
            out.push_str(&format!("  {}/s", amount(rate, unit)));
        }
        if let Some(eta) = progress.eta_s {
            out.push_str(&format!("  ~{}", duration(eta)));
        }
    }
    out.push_str(&format!(
        "  {}  rss {}",
        duration(progress.elapsed_s),
        human_bytes(progress.rss_bytes)
    ));
    match progress.state {
        JobState::Running => {}
        JobState::Finished => out.push_str("  finished"),
        JobState::Failed => out.push_str("  failed"),
    }
    out
}

/// A bar of the step's count followed by two spaces, or nothing for a step that counts nothing.
fn bar(progress: &Progress) -> String {
    let (Some(done), Some(total)) = (progress.done, progress.total) else {
        return String::new();
    };
    let share = match total {
        0 => 1.0,
        total => (done as f64 / total as f64).clamp(0.0, 1.0),
    };
    let filled = (share * BAR_CELLS as f64).round() as usize;
    format!(
        "[{}{}] {:>3.0}%  ",
        "#".repeat(filled),
        "-".repeat(BAR_CELLS - filled),
        share * 100.0
    )
}

/// What a count is of, after a space, and nothing for bytes, which carry their unit.
fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Items => " items",
        Unit::Rows => " rows",
        Unit::Pairs => " pairs",
        Unit::Terms => " terms",
        Unit::Bytes => "",
        Unit::Pages => " pages",
    }
}

fn amount(value: f64, unit: Unit) -> String {
    match unit {
        Unit::Bytes => human_bytes(value as u64),
        _ => count(value),
    }
}

/// A count to three significant figures with a decimal suffix, such as `41.2M`.
fn count(mut value: f64) -> String {
    const SUFFIXES: [&str; 6] = ["", "k", "M", "G", "T", "P"];
    let mut at = 0;
    while value >= 999.5 && at + 1 < SUFFIXES.len() {
        value /= 1000.0;
        at += 1;
    }
    let decimals = match value {
        _ if at == 0 && value.fract() == 0.0 => 0,
        v if v >= 99.95 => 0,
        v if v >= 9.995 => 1,
        _ => 2,
    };
    format!("{value:.decimals$}{}", SUFFIXES[at])
}

/// Seconds as `18s`, `7m12s` or `3h04m`.
fn duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m{:02}s", s / 60, s % 60),
        _ => format!("{}h{:02}m", s / 3600, s / 60 % 60),
    }
}

/// The time of day in UTC, as `12:04:31Z`.
fn clock(now: SystemTime) -> String {
    let s = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) % 86_400;
    format!("{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}

/// A job's tracker and the renderer its snapshots go to.
///
/// Every snapshot is taken and shown under the renderer's lock, by the job as a step ends or
/// begins and by [`show`]'s thread at the renderer's interval, so the renderer sees them in the
/// order they were taken.
pub(crate) struct Feed {
    tracker: Tracker,
    renderer: Mutex<Renderer>,
}

impl Feed {
    pub(crate) fn new(tracker: Tracker, mode: Mode) -> Feed {
        Feed {
            tracker,
            renderer: Mutex::new(Renderer::new(mode)),
        }
    }

    pub(crate) fn tracker(&self) -> &Tracker {
        &self.tracker
    }

    pub(crate) fn step_ended(&self) {
        self.with(Renderer::step_ended);
    }

    pub(crate) fn step_begun(&self) {
        self.with(Renderer::step_begun);
    }

    fn with(&self, show: impl FnOnce(&mut Renderer, &Progress)) {
        let mut renderer = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
        show(&mut renderer, &self.tracker.snapshot());
    }
}

/// Run `job` on this thread while another gives `feed`'s renderer a snapshot at its interval,
/// then mark the job finished or failed by what `job` returned, or failed where it panicked, and
/// print its last snapshot.
pub(crate) fn show<T, E>(feed: &Feed, job: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let interval = feed
        .renderer
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .interval();
    let stop = AtomicBool::new(false);
    let outcome = std::thread::scope(|scope| {
        let ticker = scope.spawn(|| loop {
            std::thread::park_timeout(interval);
            if stop.load(Ordering::Relaxed) {
                break;
            }
            feed.with(Renderer::tick);
        });
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        stop.store(true, Ordering::Relaxed);
        ticker.thread().unpark();
        outcome
    });
    let finished = matches!(outcome, Ok(Ok(_)));
    feed.tracker.end(match finished {
        true => JobState::Finished,
        false => JobState::Failed,
    });
    feed.with(Renderer::finish);
    outcome.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}
