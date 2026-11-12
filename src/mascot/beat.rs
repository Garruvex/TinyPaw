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

/// Band edges for the bass-weighted onsets: a low band (kicks, bass), a mid
/// band, and a high band (hats, snares, sibilance).
const LOW_HZ: f32 = 150.0;
const MID_HZ: f32 = 2000.0;
/// How much each band's onsets count: kicks mark the beat, hats subdivide it.
const BAND_WEIGHTS: [f32; 3] = [2.0, 1.0, 0.6];
/// Band energy is compressed as `ln(1 + COMPRESS * e)`, so quiet bands add
/// nothing and a loud hit counts for more than a faint tick.
const COMPRESS: f32 = 1000.0;
/// Extra weight for a candidate tempo near the current one (sticky tempo).
const STICKY_BONUS: f32 = 0.3;
/// Cumulative score: how much of the score comes from the past, and how
/// tightly the previous beat is expected one period back (Stark's BTrack).
const CUM_ALPHA: f32 = 0.9;
const CUM_TIGHTNESS: f32 = 5.0;
const CUM_LEN: usize = 400;

/// Which beat-tracking recipe to try. Each one adds a single mechanism to the
/// one before, so they can be compared by ear on the same music.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// High-passed loudness onsets, autocorrelation tempo, comb phase.
    Classic,
    /// Onsets from three bands with the low band weighted up.
    Bass,
    /// Bass, plus tempo that counts harmonics and resists octave flips.
    Sticky,
    /// Sticky, plus a cumulative score that carries the phase from beat to
    /// beat (BTrack-style).
    Cumulative,
}

impl Engine {
    pub const ALL: [Engine; 4] = [
        Engine::Classic,
        Engine::Bass,
        Engine::Sticky,
        Engine::Cumulative,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Engine::Classic => "classic",
            Engine::Bass => "bass",
            Engine::Sticky => "sticky",
            Engine::Cumulative => "cumulative",
        }
    }

    pub fn parse(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|e| e.id() == id)
            .unwrap_or(Engine::Bass)
    }

    fn bands(self) -> bool {
        self != Engine::Classic
    }

    fn sticky(self) -> bool {
        matches!(self, Engine::Sticky | Engine::Cumulative)
    }

    fn cumulative(self) -> bool {
        self == Engine::Cumulative
    }
}

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
    engine: Engine,
    hop: usize,
    filled: usize,
    energy: f32,
    previous: f32,
    /// Band filter states and the band energies of the current frame.
    low_a: f32,
    mid_a: f32,
    pole: [f32; 3],
    band_energy: [f32; 3],
    recent_bands: VecDeque<[f32; 3]>,
    /// Cumulative score history (Cumulative engine only).
    cum: VecDeque<f32>,
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
    pub fn new(rate: u32, sensitivity: Sensitivity, engine: Engine) -> Self {
        let alpha = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / rate as f32).exp();
        Self {
            engine,
            hop: ((rate as f64 * FRAME).round() as usize).max(1),
            filled: 0,
            energy: 0.0,
            previous: 0.0,
            low_a: alpha(LOW_HZ),
            mid_a: alpha(MID_HZ),
            pole: [0.0; 3],
            band_energy: [0.0; 3],
            recent_bands: VecDeque::with_capacity(4),
            cum: VecDeque::with_capacity(CUM_LEN + 1),
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
            if self.engine.bands() {
                // Two poles for the low band, one for the mid/high split.
                self.pole[0] += self.low_a * (s - self.pole[0]);
                self.pole[1] += self.low_a * (self.pole[0] - self.pole[1]);
                self.pole[2] += self.mid_a * (s - self.pole[2]);
                let low = self.pole[1];
                let mid = self.pole[2] - low;
                let high = s - self.pole[2];
                self.band_energy[0] += low * low;
                self.band_energy[1] += mid * mid;
                self.band_energy[2] += high * high;
            }
            self.filled += 1;
            if self.filled == self.hop {
                let n = self.hop as f32;
                let e = self.energy / n;
                let bands = self.band_energy.map(|b| b / n);
                self.filled = 0;
                self.energy = 0.0;
                self.band_energy = [0.0; 3];
                self.frame(e, bands);
            }
        }
    }

    /// Weighted rises in compressed band loudness.
    fn band_onset(&mut self, bands: [f32; 3]) -> f32 {
        let level = bands.map(|e| (1.0 + COMPRESS * e).ln());
        let count = self.recent_bands.len().max(1) as f32;
        let mut onset = 0.0;
        for b in 0..3 {
            let base = if self.recent_bands.is_empty() {
                level[b]
            } else {
                self.recent_bands.iter().map(|r| r[b]).sum::<f32>() / count
            };
            onset += BAND_WEIGHTS[b] * (level[b] - base).max(0.0);
        }
        if self.recent_bands.len() == 3 {
            self.recent_bands.pop_front();
        }
        self.recent_bands.push_back(level);
        onset
    }

    /// Beats emitted since the last call, as analysis times in seconds.
    pub fn take_beats(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.beats)
    }

    fn frame(&mut self, energy: f32, bands: [f32; 3]) {
        self.time += FRAME;
        let mut onset = if self.engine.bands() {
            self.band_onset(bands)
        } else {
            let level = (energy + 1e-10).ln();
            let base = if self.recent.is_empty() {
                level
            } else {
                self.recent.iter().sum::<f32>() / self.recent.len() as f32
            };
            let onset = if energy > 1e-7 {
                (level - base).max(0.0)
            } else {
                0.0
            };
            if self.recent.len() == 3 {
                self.recent.pop_front();
            }
            self.recent.push_back(level);
            onset
        };
        if self.time < self.quiet_until {
            onset = 0.0;
        }
        if self.onsets.len() == HISTORY {
            self.onsets.pop_front();
        }
        self.onsets.push_back(onset);
        if self.engine.cumulative() {
            self.update_cumulative(onset);
        }

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
                self.next_beat = Some(match self.project(period) {
                    Some(k) if self.engine.cumulative() => {
                        self.time + k as f64 * FRAME - DETECTION_LAG
                    }
                    _ => next + step,
                });
            }
        }

        self.since_estimate += 1;
        if self.since_estimate >= ESTIMATE_EVERY {
            self.since_estimate = 0;
            self.estimate();
        }
    }

    /// How strongly a beat `v` frames before a score is expected, for beat
    /// period `t`: a log-Gaussian around one period back.
    fn expected(v: usize, t: f32) -> f32 {
        (-0.5 * (CUM_TIGHTNESS * (v as f32 / t).ln()).powi(2)).exp()
    }

    /// The best earlier score reachable from `scores[at - v]`, over every
    /// `v` from half to twice the period, weighted by how expected it is.
    fn best_before(scores: &[f32], t: f32) -> f32 {
        let lo = ((t * 0.5).round() as usize).max(1);
        let hi = (t * 2.0).round() as usize;
        (lo..=hi)
            .filter_map(|v| {
                let i = scores.len().checked_sub(v)?;
                Some(scores[i] * Self::expected(v, t))
            })
            .fold(0.0, f32::max)
    }

    /// Adds this frame to the cumulative score: its onset, plus the best
    /// score about one beat period before it.
    fn update_cumulative(&mut self, onset: f32) {
        let value = match self.period {
            Some(t) => {
                let history: Vec<f32> = self.cum.iter().copied().collect();
                (1.0 - CUM_ALPHA) * onset + CUM_ALPHA * Self::best_before(&history, t)
            }
            None => onset,
        };
        if self.cum.len() == CUM_LEN {
            self.cum.pop_front();
        }
        self.cum.push_back(value);
    }

    /// Frames from now to the next beat: the cumulative score is run forward
    /// with no further onsets, and the strongest point near one period ahead
    /// wins.
    fn project(&self, t: f32) -> Option<usize> {
        if !self.engine.cumulative() || self.cum.len() < (t * 2.0) as usize {
            return None;
        }
        let mut scores: Vec<f32> = self.cum.iter().copied().collect();
        let reach = (t * 1.5).ceil() as usize;
        let mut best = (0, f32::MIN);
        for k in 1..=reach {
            let value = CUM_ALPHA * Self::best_before(&scores, t);
            scores.push(value);
            if k as f32 >= t * 0.5 {
                let centre = (-0.5 * ((k as f32 - t) / (t * 0.25)).powi(2)).exp();
                if value * centre > best.1 {
                    best = (k, value * centre);
                }
            }
        }
        (best.0 > 0).then_some(best.0)
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
        let sticky = self.engine.sticky();
        let current = self.period;
        let scores: Vec<(usize, f32, f32)> = (min_lag..=max_lag)
            .map(|lag| {
                let r = corr(lag);
                let octave = (lag as f32 / PREFERRED_LAG).log2();
                let mut weight = (-0.5 * (octave / 0.9).powi(2)).exp();
                let mut evidence = r;
                if sticky {
                    // A real pulse also correlates at two and four periods.
                    for (k, share) in [(2, 0.5), (4, 0.25)] {
                        if lag * k < n {
                            evidence += share * corr(lag * k);
                        }
                    }
                    // Changing tempo needs clearly more evidence than staying.
                    if let Some(p) = current {
                        let off = (lag as f32 / p).ln();
                        weight *= 1.0 + STICKY_BONUS * (-0.5 * (off / 0.06).powi(2)).exp();
                    }
                }
                (lag, r, evidence * weight)
            })
            .collect();
        let Some(&(lag, r, _)) = scores.iter().max_by(|a, b| a.2.total_cmp(&b.2)) else {
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
                    sum += o[i]
                        + 0.5 * (o.get(i + 1).copied().unwrap_or(0.0) + o[i.saturating_sub(1)]);
                    k += 1.0;
                }
                (phase, sum)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(0, |(phase, _)| phase);
        let step = p as f64 * FRAME;
        // The cumulative score keeps its own phase while the tempo holds.
        if self.engine.cumulative()
            && self.next_beat.is_some()
            && self.period.is_some_and(|old| (old - p).abs() < old * 0.05)
        {
            self.period = Some(p);
            return;
        }
        let last = self.time - best as f64 * FRAME - DETECTION_LAG;
        let mut next = last + step;
        while next < self.time {
            next += step;
        }
        self.next_beat = Some(match self.next_beat {
            // Small corrections ease in; a new tempo or phase re-locks.
            Some(current) if (next - current).abs() < step * 0.2 => {
                current + (next - current) * 0.5
            }
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
        run_with(samples, s, Engine::Classic)
    }

    pub fn run_with(samples: &[f32], s: Sensitivity, engine: Engine) -> Vec<f64> {
        let mut t = Tracker::new(RATE, s, engine);
        let mut beats = Vec::new();
        for chunk in samples.chunks(441) {
            t.push(chunk);
            beats.extend(t.take_beats());
        }
        beats
    }

    /// A low, soft kick: a decaying sine sweeping down from 90 to 50 Hz.
    fn kick(out: &mut [f32], at: f64, amplitude: f32) {
        let start = (at * RATE as f64) as usize;
        let len = (0.18 * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..len {
            let t = i as f32 / RATE as f32;
            let freq = 50.0 + 40.0 * (-t / 0.03).exp();
            phase += std::f32::consts::TAU * freq / RATE as f32;
            if let Some(s) = out.get_mut(start + i) {
                *s += amplitude * phase.sin() * (-t / 0.06).exp();
            }
        }
    }

    /// Harder music-like patterns; the true beats are the kicks.
    pub fn pattern(name: &str, bpm: f64, secs: f64) -> (Vec<f32>, Vec<f64>) {
        let mut out = vec![0.0; (secs * RATE as f64) as usize];
        let mut noise = Noise(31);
        let step = 60.0 / bpm;
        let mut truth = Vec::new();
        if name == "sustained bass" {
            for (i, s) in out.iter_mut().enumerate() {
                *s += 0.25 * (std::f32::consts::TAU * 55.0 * i as f32 / RATE as f32).sin();
            }
        }
        let mut t = 0.0;
        while t < secs {
            match name {
                // A quiet kick under louder hats on every off-beat and 16th.
                "quiet kick, loud hats" => {
                    kick(&mut out, t, 0.3);
                    for k in 1..4 {
                        hit(&mut out, t + step * k as f64 / 4.0, 0.6, 6.0, &mut noise);
                    }
                }
                // Swing: a softer hit two thirds of the way to the next beat.
                "swing" => {
                    hit(&mut out, t, 0.8, 15.0, &mut noise);
                    hit(&mut out, t + step * 2.0 / 3.0, 0.4, 10.0, &mut noise);
                }
                _ => {
                    kick(&mut out, t, 0.5);
                    hit(&mut out, t + step / 2.0, 0.4, 8.0, &mut noise);
                }
            }
            truth.push(t);
            t += step;
        }
        (out, truth)
    }

    /// Every engine on the same patterns: `cargo test engine_trial --
    /// --nocapture`. Informational, apart from silence and speech.
    #[test]
    fn engine_trial() {
        let n = Sensitivity::Normal;
        let cases = [
            pattern("kick and hats", 120.0, 24.0),
            pattern("quiet kick, loud hats", 120.0, 24.0),
            pattern("sustained bass", 100.0, 24.0),
            pattern("swing", 110.0, 24.0),
            pattern("kick and hats", 170.0, 24.0),
            pattern("kick and hats", 75.0, 24.0),
        ];
        let names = [
            "kick+hats 120",
            "quiet kick, loud hats 120",
            "sustained bass 100",
            "swing 110",
            "kick+hats 170",
            "kick+hats 75",
        ];
        print!("| Pattern |");
        for e in Engine::ALL {
            print!(" {} |", e.id());
        }
        println!("\n| --- | --- | --- | --- | --- |");
        for (name, (audio, truth)) in names.iter().zip(&cases) {
            print!("| {name} |");
            let total = truth.iter().filter(|t| **t >= 6.0).count();
            for e in Engine::ALL {
                let s = score(&run_with(audio, n, e), truth, 6.0);
                print!(
                    " {}/{total}, +{} extra, {:+.0} ms |",
                    s.hits, s.extra, s.late_ms
                );
            }
            println!();
        }
        for e in Engine::ALL {
            let speech = run_with(&speech(20.0), n, e);
            let silence = run_with(&vec![0.0; 20 * RATE as usize], n, e);
            println!(
                "{}: speech {} beats, silence {} beats",
                e.id(),
                speech.len(),
                silence.len()
            );
            assert!(silence.is_empty(), "{}: silence makes no beats", e.id());
            assert!(
                speech.len() <= 4,
                "{}: speech makes {}",
                e.id(),
                speech.len()
            );
        }
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
            late_ms: if hits > 0 {
                late / hits as f64 * 1000.0
            } else {
                0.0
            },
        }
    }

    #[test]
    fn beat_trial() {
        let n = Sensitivity::Normal;
        let mut rows = Vec::new();
        for bpm in [80.0, 120.0, 160.0] {
            let (audio, truth) = clicks(&[(bpm, 20.0)], false);
            let s = score(&run(&audio, n), &truth, 4.0);
            rows.push((
                format!("{bpm:.0} BPM clicks"),
                s,
                truth.iter().filter(|t| **t >= 4.0).count(),
            ));
        }
        let (audio, truth) = clicks(&[(120.0, 20.0)], true);
        let s = score(&run(&audio, n), &truth, 4.0);
        rows.push((
            "120 BPM with off-beat hats".into(),
            s,
            truth.iter().filter(|t| **t >= 4.0).count(),
        ));
        // Tempo change at 12 s: score the second half after 4 s to re-lock.
        let (audio, truth) = clicks(&[(100.0, 12.0), (140.0, 14.0)], false);
        let s = score(&run(&audio, n), &truth, 16.0);
        rows.push((
            "100 → 140 BPM change".into(),
            s,
            truth.iter().filter(|t| **t >= 16.0).count(),
        ));
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
        println!(
            "| Speech-like bursts, 20 s | 0 | – | – | {} | – |",
            speech.len()
        );
        println!("| Silence, 20 s | 0 | – | – | {} | – |", silence.len());

        // The bar for shipping Follow beat as a trial.
        for (name, s, total) in &rows {
            assert!(s.hits * 100 >= total * 90, "{name}: {s:?} of {total}");
            assert!(s.extra * 100 <= total * 10, "{name}: {s:?}");
            assert!(s.late_ms.abs() <= 40.0, "{name}: {s:?}");
        }
        assert!(silence.is_empty(), "silence makes no beats");
        assert!(
            speech.len() <= 2,
            "speech makes almost no beats: {}",
            speech.len()
        );
    }

    #[test]
    fn own_sounds_can_be_held_off() {
        let mut t = Tracker::new(RATE, Sensitivity::Normal, Engine::Classic);
        let (audio, _) = clicks(&[(120.0, 1.0)], false);
        t.hold_off(2.0);
        t.push(&audio);
        assert!(t.onsets.iter().all(|o| *o == 0.0));
    }
}
