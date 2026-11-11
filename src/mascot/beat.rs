//! Beat following, a trial: predicts beats from playback so nods land on
//! them. Onsets come from rises in high-passed loudness; the tempo from the
//! autocorrelation of the onsets over the last few seconds (60–180 BPM,
//! preferring moderate tempos to avoid double/half-time); the phase from a
//! comb over past onsets. Beats are emitted at predicted times, so they hold
//! through off-beats and short gaps, and stop when the pattern isn't clear.

use std::collections::VecDeque;

/// Analysis frames are 10 ms.
const FRAME: f64 = 0.01;
/// Onset history used for tempo and phase: 6 s.
const HISTORY: usize = 600;
/// Re-estimate tempo and phase every 0.5 s, once 3 s are in.
const ESTIMATE_EVERY: usize = 50;
const MIN_HISTORY: usize = 300;
const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 180.0;
/// Frames per beat the tempo preference centres on (120 BPM).
const PREFERRED_LAG: f32 = 50.0;
/// An attack is seen at the end of the frame it rises in; beats are
/// predicted this much earlier to land on the attack itself.
const DETECTION_LAG: f64 = 0.015;

/// How readily the tracker commits to a beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    Low,
    Normal,
    High,
}

impl Sensitivity {
    /// The pattern clarity (0–1) needed before beats are emitted.
    fn confidence(self) -> f32 {
        match self {
            // Measured: speech-like bursts peak at 0.25, the click tracks
            // never drop below 0.45 (see the trial below).
            Sensitivity::Low => 0.5,
            Sensitivity::Normal => 0.35,
            Sensitivity::High => 0.28,
        }
    }
}

pub struct Tracker {
    hop: usize,
    filled: usize,
    energy: f32,
    previous: f32,
    /// Seconds of audio analysed.
    time: f64,
    recent: VecDeque<f32>,
    onsets: VecDeque<f32>,
    since_estimate: usize,
    /// Beat period in frames, while a tempo is known.
    period: Option<f32>,
    confidence: f32,
    next_beat: Option<f64>,
    last_beat: f64,
    /// Onsets are ignored until this time, while TinyPaw itself plays a
    /// sound (loopback hears everything, so this is approximate).
    quiet_until: f64,
    sensitivity: Sensitivity,
    beats: Vec<f64>,
}

impl Tracker {
    pub fn new(rate: u32, sensitivity: Sensitivity) -> Self {
        Self {
            hop: ((rate as f64 * FRAME).round() as usize).max(1),
            filled: 0,
            energy: 0.0,
            previous: 0.0,
            time: 0.0,
            recent: VecDeque::with_capacity(4),
            onsets: VecDeque::with_capacity(HISTORY + 1),
            since_estimate: 0,
            period: None,
            confidence: 0.0,
            next_beat: None,
            last_beat: f64::NEG_INFINITY,
            quiet_until: 0.0,
            sensitivity,
            beats: Vec::new(),
        }
    }

    pub fn set_sensitivity(&mut self, s: Sensitivity) {
        self.sensitivity = s;
    }

    /// The tempo, while the beat is clear enough to follow.
    pub fn bpm(&self) -> Option<f32> {
        self.period
            .filter(|_| self.confident())
            .map(|p| 60.0 / (p * FRAME as f32))
    }

    fn confident(&self) -> bool {
        self.confidence >= self.sensitivity.confidence()
    }

    /// Ignore onsets for `seconds` from now, while TinyPaw plays a sound.
    pub fn hold_off(&mut self, seconds: f64) {
        self.quiet_until = self.quiet_until.max(self.time + seconds);
    }

    /// Feeds mono samples in order.
    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            // A first difference favours the attacks that mark beats.
            let hp = s - self.previous;
            self.previous = s;
            self.energy += hp * hp;
            self.filled += 1;
            if self.filled == self.hop {
                let e = self.energy / self.hop as f32;
                self.filled = 0;
                self.energy = 0.0;
                self.frame(e);
            }
        }
    }

    /// Beats emitted since the last call, as analysis times in seconds.
    pub fn take_beats(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.beats)
    }

    fn frame(&mut self, energy: f32) {
        self.time += FRAME;
        let level = (energy + 1e-10).ln();
        let base = if self.recent.is_empty() {
            level
        } else {
            self.recent.iter().sum::<f32>() / self.recent.len() as f32
        };
        let mut onset = if energy > 1e-7 { (level - base).max(0.0) } else { 0.0 };
        if self.time < self.quiet_until {
            onset = 0.0;
        }
        if self.recent.len() == 3 {
            self.recent.pop_front();
        }
        self.recent.push_back(level);
        if self.onsets.len() == HISTORY {
            self.onsets.pop_front();
        }
        self.onsets.push_back(onset);

        // A beat due now is emitted before any re-estimate can move the
        // prediction past it.
        if let (Some(next), Some(period)) = (self.next_beat, self.period) {
            let step = period as f64 * FRAME;
            if self.time >= next {
                // Never twice for one beat, even after a re-estimate.
                if self.confident() && self.time - self.last_beat > step * 0.5 {
                    self.beats.push(self.time);
                    self.last_beat = self.time;
                }
                self.next_beat = Some(next + step);
            }
        }

        self.since_estimate += 1;
        if self.since_estimate >= ESTIMATE_EVERY {
            self.since_estimate = 0;
            self.estimate();
        }
    }

    fn estimate(&mut self) {
        let n = self.onsets.len();
        if n < MIN_HISTORY {
            return;
        }
        // Smoothed over neighbouring frames, so a period that falls between
        // whole frames still correlates fully.
        let raw: Vec<f32> = self.onsets.iter().copied().collect();
        let o: Vec<f32> = (0..n)
            .map(|i| {
                0.5 * raw[i]
                    + 0.25 * (raw[i.saturating_sub(1)] + raw.get(i + 1).copied().unwrap_or(0.0))
            })
            .collect();
        let mean = o.iter().sum::<f32>() / n as f32;
        let centred: Vec<f32> = o.iter().map(|v| v - mean).collect();
        let zero: f32 = centred.iter().map(|v| v * v).sum();
        if zero < 1e-6 {
            self.period = None;
            self.confidence = 0.0;
            self.next_beat = None;
            return;
        }
        let min_lag = (60.0 / (MAX_BPM * FRAME as f32)).floor() as usize;
        let max_lag = ((60.0 / (MIN_BPM * FRAME as f32)).ceil() as usize).min(n / 2);
        let corr = |lag: usize| -> f32 {
            centred[lag..]
                .iter()
                .zip(&centred[..n - lag])
                .map(|(a, b)| a * b)
                .sum()
        };
        let scores: Vec<(usize, f32, f32)> = (min_lag..=max_lag)
            .map(|lag| {
                let r = corr(lag);
                let octave = (lag as f32 / PREFERRED_LAG).log2();
                let weight = (-0.5 * (octave / 0.9).powi(2)).exp();
                (lag, r, r * weight)
            })
            .collect();
        let Some(&(lag, r, _)) = scores
            .iter()
            .max_by(|a, b| a.2.total_cmp(&b.2))
        else {
            return;
        };
        // Parabolic interpolation for a fractional period.
        let at = |l: usize| scores.iter().find(|s| s.0 == l).map(|s| s.1);
        let period = match (at(lag.wrapping_sub(1)), at(lag + 1)) {
            (Some(a), Some(c)) if (a - 2.0 * r + c).abs() > 1e-9 => {
                lag as f32 + 0.5 * (a - c) / (a - 2.0 * r + c)
            }
            _ => lag as f32,
        };
        self.confidence = (r / zero).max(0.0);

        // Phase: where combs of past beats collect the most onset.
        let p = period;
        let best = (0..lag)
            .map(|phase| {
                let mut sum = 0.0;
                let mut k = 0.0;
                loop {
                    let back = phase as f32 + k * p;
                    let i = n as f32 - 1.0 - back;
                    if i < 0.0 {
                        break;
                    }
                    // Spread over neighbours, so a fractional period lines up.
                    let i = i.round() as usize;
                    sum += o[i] + 0.5 * (o.get(i + 1).copied().unwrap_or(0.0) + o[i.saturating_sub(1)]);
                    k += 1.0;
                }
                (phase, sum)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(0, |(phase, _)| phase);
        let step = p as f64 * FRAME;
        let last = self.time - best as f64 * FRAME - DETECTION_LAG;
        let mut next = last + step;
        while next < self.time {
            next += step;
        }
        self.next_beat = Some(match self.next_beat {
            // Small corrections ease in; a new tempo or phase re-locks.
            Some(current) if (next - current).abs() < step * 0.2 => current + (next - current) * 0.5,
            _ => next,
        });
        self.period = Some(p);
    }
}

#[cfg(test)]
pub(crate) mod trial {
    //! The click-track trial the plan asks for before Follow beat ships.
    //! Results are printed by `cargo test beat_trial -- --nocapture` and
    //! recorded in docs/mascot-feature.md.
    use super::*;

    pub const RATE: u32 = 44_100;

    struct Noise(u32);
    impl Noise {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    fn hit(out: &mut [f32], at: f64, amplitude: f32, millis: f32, noise: &mut Noise) {
        let start = (at * RATE as f64) as usize;
        let len = (millis / 1000.0 * RATE as f32) as usize;
        for i in 0..len {
            if let Some(s) = out.get_mut(start + i) {
                let decay = (-(i as f32) / (RATE as f32 * 0.004)).exp();
                *s += noise.next() * amplitude * decay;
            }
        }
    }

    /// A click on every beat across tempo `segments`; with `hats`, quieter
    /// clicks on the off-beats too. Returns samples and true beat times.
    pub fn clicks(segments: &[(f32, f64)], hats: bool) -> (Vec<f32>, Vec<f64>) {
        let total: f64 = segments.iter().map(|s| s.1).sum();
        let mut out = vec![0.0; (total * RATE as f64) as usize];
        let mut noise = Noise(7);
        let mut beats = Vec::new();
        let mut t = 0.0;
        let mut end = 0.0;
        for &(bpm, secs) in segments {
            end += secs;
            let step = 60.0 / bpm as f64;
            while t < end {
                hit(&mut out, t, 0.8, 15.0, &mut noise);
                if hats {
                    hit(&mut out, t + step / 2.0, 0.3, 8.0, &mut noise);
                }
                beats.push(t);
                t += step;
            }
        }
        (out, beats)
    }

    /// Syllable-like bursts at irregular gaps: no beat to find.
    pub fn speech(secs: f64) -> Vec<f32> {
        let mut out = vec![0.0; (secs * RATE as f64) as usize];
        let mut noise = Noise(99);
        let mut t = 0.2;
        while t < secs {
            let len = 80.0 + (noise.next().abs() * 170.0);
            let amp = 0.3 + noise.next().abs() * 0.3;
            let start = (t * RATE as f64) as usize;
            let n = (len / 1000.0 * RATE as f32) as usize;
            for i in 0..n {
                if let Some(s) = out.get_mut(start + i) {
                    let env = (std::f32::consts::PI * i as f32 / n as f32).sin();
                    *s += noise.next() * amp * env;
                }
            }
            t += (len as f64 + 150.0 + noise.next().abs() as f64 * 350.0) / 1000.0;
        }
        out
    }

    /// Feeds `samples` in 10 ms chunks and returns the emitted beat times.
    pub fn run(samples: &[f32], s: Sensitivity) -> Vec<f64> {
        let mut t = Tracker::new(RATE, s);
        let mut beats = Vec::new();
        for chunk in samples.chunks(441) {
            t.push(chunk);
            beats.extend(t.take_beats());
        }
        beats
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Score {
        pub hits: usize,
        pub missed: usize,
        pub extra: usize,
        /// Mean emitted-minus-true time of hits, in milliseconds.
        pub late_ms: f64,
    }

    /// Matches emitted beats to true ones within ±70 ms, counting from
    /// `from` seconds (after the tracker has had time to lock on).
    pub fn score(emitted: &[f64], truth: &[f64], from: f64) -> Score {
        let mut used = vec![false; emitted.len()];
        let (mut hits, mut late) = (0, 0.0);
        let truth: Vec<f64> = truth.iter().copied().filter(|t| *t >= from).collect();
        for t in &truth {
            if let Some((i, e)) = emitted
                .iter()
                .enumerate()
                .filter(|(i, e)| !used[*i] && (**e - t).abs() <= 0.07)
                .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()))
            {
                used[i] = true;
                hits += 1;
                late += e - t;
            }
        }
        let extra = emitted
            .iter()
            .enumerate()
            .filter(|(i, e)| **e >= from && !used[*i])
            .count();
        Score {
            hits,
            missed: truth.len() - hits,
            extra,
            late_ms: if hits > 0 { late / hits as f64 * 1000.0 } else { 0.0 },
        }
    }

    #[test]
    fn beat_trial() {
        let n = Sensitivity::Normal;
        let mut rows = Vec::new();
        for bpm in [80.0, 120.0, 160.0] {
            let (audio, truth) = clicks(&[(bpm, 20.0)], false);
            let s = score(&run(&audio, n), &truth, 4.0);
            rows.push((format!("{bpm:.0} BPM clicks"), s, truth.iter().filter(|t| **t >= 4.0).count()));
        }
        let (audio, truth) = clicks(&[(120.0, 20.0)], true);
        let s = score(&run(&audio, n), &truth, 4.0);
        rows.push(("120 BPM with off-beat hats".into(), s, truth.iter().filter(|t| **t >= 4.0).count()));
        // Tempo change at 12 s: score the second half after 4 s to re-lock.
        let (audio, truth) = clicks(&[(100.0, 12.0), (140.0, 14.0)], false);
        let s = score(&run(&audio, n), &truth, 16.0);
        rows.push(("100 → 140 BPM change".into(), s, truth.iter().filter(|t| **t >= 16.0).count()));
        let speech = run(&speech(20.0), n);
        let silence = run(&vec![0.0; 20 * RATE as usize], n);

        println!("| Signal | Beats | Hits | Missed | Extra | Mean offset |");
        println!("| --- | --- | --- | --- | --- | --- |");
        for (name, s, total) in &rows {
            println!(
                "| {name} | {total} | {} | {} | {} | {:+.0} ms |",
                s.hits, s.missed, s.extra, s.late_ms
            );
        }
        println!("| Speech-like bursts, 20 s | 0 | – | – | {} | – |", speech.len());
        println!("| Silence, 20 s | 0 | – | – | {} | – |", silence.len());

        // The bar for shipping Follow beat as a trial.
        for (name, s, total) in &rows {
            assert!(s.hits * 100 >= total * 90, "{name}: {s:?} of {total}");
            assert!(s.extra * 100 <= total * 10, "{name}: {s:?}");
            assert!(s.late_ms.abs() <= 40.0, "{name}: {s:?}");
        }
        assert!(silence.is_empty(), "silence makes no beats");
        assert!(speech.len() <= 2, "speech makes almost no beats: {}", speech.len());
    }

    #[test]
    fn own_sounds_can_be_held_off() {
        let mut t = Tracker::new(RATE, Sensitivity::Normal);
        let (audio, _) = clicks(&[(120.0, 1.0)], false);
        t.hold_off(2.0);
        t.push(&audio);
        assert!(t.onsets.iter().all(|o| *o == 0.0));
    }
}


