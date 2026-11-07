//! A long-running job's progress on stderr, drawn from [`Progress`] snapshots.
//!
//! [`Renderer`] takes snapshots from any source and decides what to print for the chosen
//! [`Mode`]. [`Feed`] and [`show`] are the source for a job that runs in this process and counts
//! into a [`Tracker`]: a thread snapshots the tracker at the renderer's interval, and the job
//! queues the first snapshot of each step as the step begins, so a step shorter than the interval
//! is still printed.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

impl Mode {
    /// `Auto` decided against stderr.
    fn resolved(self) -> Mode {
        match self {
            Mode::Auto if std::io::IsTerminal::is_terminal(&std::io::stderr()) => Mode::Bar,
            Mode::Auto => Mode::Plain,
            mode => mode,
        }
    }
}

const PLAIN_EVERY: Duration = Duration::from_secs(30);
const JSON_EVERY: Duration = Duration::from_secs(1);
const BAR_EVERY: Duration = Duration::from_millis(250);
const BAR_CELLS: usize = 24;

/// Decides which snapshots to print and prints them.
pub(crate) struct Renderer {
    mode: Mode,
    /// The step of the last snapshot printed: its phase, name and context.
    shown: Option<(u32, String, Option<String>)>,
    printed: Option<Instant>,
    /// Whether the bar's two lines are on the screen, with the cursor at the start of the first.
    drawn: bool,
}

impl Renderer {
    pub(crate) fn new(mode: Mode) -> Renderer {
        Renderer {
            mode: mode.resolved(),
            shown: None,
            printed: None,
            drawn: false,
        }
    }

    /// How often a reader should take a snapshot for this renderer.
    pub(crate) fn interval(&self) -> Duration {
        match self.mode {
            Mode::Bar => BAR_EVERY,
            _ => JSON_EVERY,
        }
    }

    /// Print `progress` if the mode asks for it now: on a new step, and otherwise when the mode's
    /// interval has passed since the last print.
    pub(crate) fn show(&mut self, progress: &Progress) {
        let step = (
            progress.phase.index,
            progress.step.to_string(),
            progress.context.clone(),
        );
        let new_step = self.shown.as_ref() != Some(&step);
        let since = self.printed.map(|at| at.elapsed());
        let due = match self.mode {
            Mode::Plain => new_step || since.is_none_or(|s| s >= PLAIN_EVERY),
            Mode::Json => new_step || since.is_none_or(|s| s >= JSON_EVERY),
            Mode::Bar => since.is_none_or(|s| s >= BAR_EVERY / 2),
            Mode::Auto | Mode::None => false,
        };
        if due {
            self.print(progress);
            self.shown = Some(step);
            self.printed = Some(Instant::now());
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
                // The last draw left the cursor at the start of the bar's first line, so anything
                // the job has written to stderr since then is written over the bar. Clearing from
                // the cursor to the end of the screen removes what is left of the old bar, and the
                // new one is drawn below the job's lines.
                let clear = "\x1b[J";
                let drawn = write!(
                    err,
                    "{clear}[{}/{}] {}\n{}{}\n\x1b[2A\r",
                    progress.phase.index,
                    progress.phase.of,
                    progress.phase.name,
                    bar(progress),
                    line(progress)
                );
                self.drawn = true;
                drawn
            }
            Mode::Auto | Mode::None => Ok(()),
        };
        let _ = err.flush();
    }
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
        bytes(progress.rss_bytes as f64)
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
        Unit::Bytes => bytes(value),
        _ => count(value),
    }
}

/// A count to three significant figures with a decimal suffix, such as `41.2M`.
fn count(value: f64) -> String {
    scaled(value, 1000.0, &["", "k", "M", "G", "T", "P"], "")
}

/// A byte count to three significant figures in binary units, such as `18.2 GiB`.
fn bytes(value: f64) -> String {
    scaled(
        value,
        1024.0,
        &["B", "KiB", "MiB", "GiB", "TiB", "PiB"],
        " ",
    )
}

fn scaled(mut value: f64, base: f64, suffixes: &[&str], space: &str) -> String {
    let mut at = 0;
    while value >= base * 0.9995 && at + 1 < suffixes.len() {
        value /= base;
        at += 1;
    }
    if at == 0 && value.fract() == 0.0 {
        return format!("{value:.0}{space}{}", suffixes[0]);
    }
    let decimals = match value {
        v if v >= 99.95 => 0,
        v if v >= 9.995 => 1,
        _ => 2,
    };
    format!("{value:.decimals$}{space}{}", suffixes[at])
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

/// A job's tracker and the queue of its snapshots, which [`show`] draws.
///
/// Every snapshot is taken and queued under one lock, by the job as a step begins and by the
/// drawing thread when its interval passes, so the queue holds them in the order they were taken
/// and the step never appears to go back.
pub(crate) struct Feed {
    tracker: Tracker,
    sender: Mutex<Sender<Option<Progress>>>,
    receiver: Mutex<Receiver<Option<Progress>>>,
}

impl Feed {
    pub(crate) fn new(tracker: Tracker) -> Feed {
        let (sender, receiver) = mpsc::channel();
        Feed {
            tracker,
            sender: Mutex::new(sender),
            receiver: Mutex::new(receiver),
        }
    }

    pub(crate) fn tracker(&self) -> &Tracker {
        &self.tracker
    }

    /// Queue a snapshot of the job as it is now, such as when a step has just begun.
    pub(crate) fn queue(&self) {
        let sender = self.sender.lock().unwrap_or_else(|p| p.into_inner());
        let _ = sender.send(Some(self.tracker.snapshot()));
    }

    fn stop(&self) {
        let sender = self.sender.lock().unwrap_or_else(|p| p.into_inner());
        let _ = sender.send(None);
    }
}

/// Run `job` on this thread while another draws `feed`'s snapshots in `mode`, then mark the job
/// finished or failed by what `job` returned and print its last snapshot.
pub(crate) fn show<T, E>(
    mode: Mode,
    feed: &Feed,
    job: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let mut renderer = Renderer::new(mode);
    let outcome = std::thread::scope(|scope| {
        let renderer = &mut renderer;
        scope.spawn(move || {
            let receiver = feed.receiver.lock().unwrap_or_else(|p| p.into_inner());
            loop {
                match receiver.recv_timeout(renderer.interval()) {
                    Ok(Some(progress)) => renderer.show(&progress),
                    Err(RecvTimeoutError::Timeout) => feed.queue(),
                    Ok(None) | Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        // Stops the drawing thread when the job returns or unwinds, so the scope can end.
        struct Stop<'a>(&'a Feed);
        impl Drop for Stop<'_> {
            fn drop(&mut self) {
                self.0.stop();
            }
        }
        let _stop = Stop(feed);
        job()
    });
    feed.tracker.end(match outcome {
        Ok(_) => JobState::Finished,
        Err(_) => JobState::Failed,
    });
    renderer.finish(&feed.tracker.snapshot());
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_read_to_three_significant_figures() {
        assert_eq!(count(41_200_000.0), "41.2M");
        assert_eq!(count(1_830_000.0), "1.83M");
        assert_eq!(count(740.0), "740");
        assert_eq!(count(999_999.0), "1.00M");
        assert_eq!(bytes(19_500_000_000.0), "18.2 GiB");
        assert_eq!(bytes(512.0), "512 B");
        assert_eq!(duration(432.0), "7m12s");
        assert_eq!(duration(17.4), "17s");
        assert_eq!(duration(3_840.0), "1h04m");
    }
}
