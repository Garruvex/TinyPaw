//! Shared, clock-driven headpat and sleep state. Platform adapters supply
//! proximity, posed hit tests and external audio status.
use iced::Point;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub petting: bool,
    pub sleep: bool,
    pub delay: f32,
    pub touch_wake: bool,
    pub audio_wake: bool,
    pub random_wake: bool,
    pub zzz: bool,
    pub breathing: bool,
    pub touch_seconds: [f32; 2],
    pub random_seconds: [f32; 2],
    pub mouth: [f32; 2],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            petting: false,
            sleep: false,
            delay: 300.0,
            touch_wake: true,
            audio_wake: true,
            random_wake: false,
            zzz: true,
            breathing: true,
            touch_seconds: [1.0, 3.0],
            random_seconds: [300.0, 900.0],
            mouth: [0.5, 0.35],
        }
    }
}
impl Settings {
    pub fn parse(v: &Value) -> Result<Self, String> {
        let mut s = Self::default();
        if v.is_null() {
            return Ok(s);
        }
        if !v.is_object() {
            return Err("companionship must be an object".into());
        }
        for (key, target) in [
            ("petting", &mut s.petting),
            ("sleep", &mut s.sleep),
            ("touch_wake", &mut s.touch_wake),
            ("audio_wake", &mut s.audio_wake),
            ("random_wake", &mut s.random_wake),
            ("zzz", &mut s.zzz),
            ("breathing", &mut s.breathing),
        ] {
            if let Some(v) = v.get(key) {
                *target = v.as_bool().ok_or(format!("{key} must be true or false"))?;
            }
        }
        if let Some(v) = v.get("delay_seconds") {
            s.delay = v
                .as_f64()
                .filter(|n| n.is_finite() && (60.0..=3600.0).contains(n))
                .ok_or("sleep delay must be 60–3600 seconds")? as f32;
        }
        if let Some(v) = v.get("mouth") {
            let a = v
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or("mouth needs two fractions")?;
            for (i, value) in a.iter().enumerate() {
                s.mouth[i] = value
                    .as_f64()
                    .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                    .ok_or("mouth needs two fractions")? as f32;
            }
        }
        for (key, target, min, max) in [
            ("touch_seconds", &mut s.touch_seconds, 0.2, 10.0),
            ("random_seconds", &mut s.random_seconds, 60.0, 7200.0),
        ] {
            if let Some(value) = v.get(key) {
                let a = value
                    .as_array()
                    .filter(|a| a.len() == 2)
                    .ok_or(format!("{key} needs a minimum and maximum"))?;
                for (index, value) in a.iter().enumerate() {
                    target[index] = value
                        .as_f64()
                        .filter(|n| n.is_finite() && *n >= min && *n <= max)
                        .ok_or(format!("invalid {key} range"))?
                        as f32;
                }
                if target[0] > target[1] {
                    return Err(format!("{key} minimum exceeds maximum"));
                }
            }
        }
        Ok(s)
    }
    pub fn to_json(&self) -> Value {
        json!({"petting":self.petting,"sleep":self.sleep,"delay_seconds":self.delay,
            "touch_wake":self.touch_wake,"audio_wake":self.audio_wake,
            "random_wake":self.random_wake,"zzz":self.zzz,"mouth":self.mouth,"breathing":self.breathing,"touch_seconds":self.touch_seconds,"random_seconds":self.random_seconds})
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audio {
    Unknown,
    Quiet,
    Playing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Awake,
    Petting,
    Asleep,
    Waking,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Headpat,
    Sleep,
    Wake,
}
pub struct Input {
    pub now: f32,
    pub cursor: Option<Point>,
    pub nearby: bool,
    pub touching: bool,
    pub pet_region: bool,
    pub button: bool,
    pub busy: bool,
    pub response_busy: bool,
    pub audio: Audio,
    pub height: f32,
}
#[derive(Debug, Default)]
pub struct Controller {
    pub state: State,
    pub stirring: bool,
    pub sleep_due: bool,
    sleep_pose_count: usize,
    pub sleep_pose: usize,
    previous_sleep_pose: Option<usize>,
    sleep_started: Option<f32>,
    wake_started: Option<f32>,
    quiet_since: Option<f32>,
    touch_since: Option<f32>,
    touch_left: Option<f32>,
    audio_since: Option<f32>,
    wake_at: Option<f32>,
    random_at: Option<f32>,
    touch_delay: f32,
    last: Option<Point>,
    stroke_start: Option<f32>,
    direction: f32,
    reversal_x: f32,
    travel: f32,
    reversals: u8,
    last_stroke: f32,
    pet_cooldown: f32,
    seed: u64,
    affection: Affection,
}
impl Controller {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: seed.max(1),
            ..Self::default()
        }
    }
    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.seed >> 40) as f32 / (1u32 << 24) as f32
    }
    pub fn pet_now(&mut self, now: f32) {
        self.affection.preview_until = now + 2.5;
        self.state = State::Petting;
        self.last_stroke = now + 2.5;
    }
    /// Shared desktop/preview movement, including a fallback for complete poses.
    pub fn apply_affection(
        &self,
        rig: &super::rig::Rig,
        pose: &mut super::motion::Pose,
        now: f32,
        reduced: bool,
        whole_picture: bool,
    ) {
        if !matches!(self.state, State::Awake | State::Petting) || reduced {
            return;
        }
        let amount = self.affection.amount;
        if amount < 0.001 {
            return;
        }
        let sway = (now * 5.0 + self.affection.phase).sin();
        let tilt = amount * (0.08 * sway + 0.09 * self.affection.direction);
        if !whole_picture {
            if let Some(index) = rig
                .motion_parts()
                .iter()
                .position(|p| p.kind == super::motion::PartType::Head)
            {
                if let Some(angle) = pose.angles.get_mut(index) {
                    *angle += tilt;
                }
                pose.head_drop += amount * (0.008 + 0.006 * (now * 7.0).sin());
                return;
            }
        }
        pose.lean += tilt * 0.35;
        pose.squash += amount * (0.08 + 0.06 * (now * 6.0).sin());
    }
    pub fn draw_affection(
        &self,
        frame: &mut iced::widget::canvas::Frame,
        stage: iced::Size,
        bounds: iced::Rectangle,
        now: f32,
        reduced: bool,
    ) {
        if !matches!(self.state, State::Awake | State::Petting) {
            return;
        }
        let size = (bounds.height * 0.085).max(12.0).min(stage.width * 0.2);
        if reduced {
            if self.state == State::Petting {
                draw_heart(
                    frame,
                    Point::new(
                        (bounds.x + bounds.width - size).clamp(0.0, stage.width - size),
                        (bounds.y - size).max(0.0),
                    ),
                    size,
                    0.8,
                );
            }
            return;
        }
        for heart in &self.affection.hearts {
            let t = (now - heart.born) / 1.5;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let x = (heart.origin.x
                + heart.side * size * (0.4 + 1.2 * t)
                + (t * 5.0).sin() * size * 0.15)
                .clamp(0.0, (stage.width - size).max(0.0));
            let y = (heart.origin.y - size * (0.5 + 2.8 * t))
                .clamp(0.0, (stage.height - size).max(0.0));
            let pop = (t / 0.12).min(1.0);
            draw_heart(
                frame,
                Point::new(x, y),
                size * (0.6 + 0.4 * pop),
                (1.0 - t) * pop * 0.9,
            );
        }
    }
    /// Shared visual timing for the desktop and settings preview.
    pub fn sleep_picture(&self, now: f32) -> bool {
        matches!(self.state, State::Asleep | State::Waking)
            && self.sleep_started.is_some_and(|at| now - at >= 2.6)
            && self.wake_started.is_none()
    }
    pub fn sleep_motion(&self, now: f32) -> bool {
        matches!(self.state, State::Asleep | State::Waking)
            || self.wake_started.is_some_and(|at| now - at < 1.4)
    }
    pub fn apply_sleep(
        &self,
        rig: &super::rig::Rig,
        pose: &mut super::motion::Pose,
        now: f32,
        reduced: bool,
        whole_picture: bool,
    ) {
        if !self.sleep_motion(now) {
            return;
        }
        pose.music_active = false;
        let ease = |t: f32| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let elapsed = self.sleep_started.map_or(2.6, |at| (now - at).max(0.0));
        let (amount, closed) = if let Some(at) = self.wake_started {
            let t = (now - at).max(0.0);
            (1.0 - ease(t / 1.4), t < 0.35 || (0.75..0.95).contains(&t))
        } else {
            (
                ease(elapsed / 2.6),
                (0.65..1.05).contains(&elapsed) || elapsed >= 1.65,
            )
        };
        pose.gaze = iced::Vector::ZERO;
        let closed = if reduced {
            matches!(self.state, State::Asleep | State::Waking)
        } else {
            closed
        };
        pose.expression = if closed {
            super::motion::Expression::Blink
        } else {
            super::motion::Expression::Idle
        };
        if reduced {
            return;
        }
        if !whole_picture
            && rig
                .motion_parts()
                .iter()
                .any(|p| p.kind == super::motion::PartType::Head)
        {
            pose.head_drop += 0.025 * amount;
        } else {
            pose.squash += 0.035 * amount;
        }
    }
    pub fn reset(&mut self) {
        let count = self.sleep_pose_count;
        let previous = self.previous_sleep_pose;
        *self = Self::new(self.seed);
        self.sleep_pose_count = count;
        self.previous_sleep_pose = previous;
    }
    pub fn wake(&mut self, now: f32) -> Option<Event> {
        let was_sleeping = matches!(self.state, State::Asleep | State::Waking);
        if was_sleeping && self.wake_started.is_none() {
            self.wake_started = Some(now);
        }
        self.state = State::Awake;
        self.quiet_since = Some(now);
        self.sleep_due = false;
        self.stirring = false;
        self.touch_since = None;
        self.touch_left = None;
        self.wake_at = None;
        self.audio_since = None;
        self.random_at = None;
        self.clear_strokes();
        was_sleeping.then_some(Event::Wake)
    }
    pub fn set_sleep_pose_count(&mut self, count: usize) {
        self.sleep_pose_count = count;
        if self.sleep_pose >= count {
            self.sleep_pose = 0;
        }
    }
    pub fn sleep_now(&mut self, now: f32, s: &Settings) {
        let count = self.sleep_pose_count;
        self.sleep_pose = if count > 1 {
            let previous = self.previous_sleep_pose.filter(|p| *p < count);
            let options = count - usize::from(previous.is_some());
            let pick = ((self.random() * options as f32) as usize).min(options - 1);
            pick + usize::from(previous.is_some_and(|p| pick >= p))
        } else {
            0
        };
        self.previous_sleep_pose = Some(self.sleep_pose);
        self.state = State::Asleep;
        self.sleep_started = Some(now);
        self.wake_started = None;
        self.touch_left = None;
        self.touch_delay =
            s.touch_seconds[0] + (s.touch_seconds[1] - s.touch_seconds[0]) * self.random();
        self.random_at = s.random_wake.then(|| {
            now + s.random_seconds[0] + (s.random_seconds[1] - s.random_seconds[0]) * self.random()
        });
        self.touch_since = None;
        self.audio_since = None;
        self.wake_at = None;
        self.stirring = false;
        self.clear_strokes();
    }
    fn clear_strokes(&mut self) {
        self.stroke_start = None;
        self.travel = 0.0;
        self.reversals = 0;
        self.direction = 0.0;
    }
    fn stroke(&mut self, i: &Input) -> bool {
        let previous = self.last;
        self.last = i.cursor;
        if !i.pet_region || i.button || i.busy {
            self.clear_strokes();
            return false;
        }
        let (Some(p), Some(last)) = (i.cursor, previous) else {
            return false;
        };
        if self.stroke_start.is_none_or(|at| i.now - at > 1.2) {
            self.clear_strokes();
            self.stroke_start = Some(i.now);
            self.reversal_x = p.x;
        }
        let scale = (i.height / 146.0).clamp(0.5, 3.0);
        self.travel += p.distance(last);
        let delta = p.x - self.reversal_x;
        if delta.abs() >= 8.0 * scale {
            if self.direction != 0.0 && delta.signum() != self.direction {
                self.reversals += 1;
            }
            self.direction = delta.signum();
            self.reversal_x = p.x;
        }
        let valid = self.travel >= 40.0 * scale && self.reversals >= 2 && p.distance(last) > 0.5;
        if valid {
            self.last_stroke = i.now;
        }
        valid
    }
    pub fn step(&mut self, s: &Settings, i: Input) -> Option<Event> {
        let previous = self.last;
        let event = self.step_state(s, &i);
        let active = s.petting
            && !i.button
            && !i.busy
            && matches!(self.state, State::Awake | State::Petting);
        self.affection
            .update(&i, previous, active, self.state == State::Petting);
        event
    }
    fn step_state(&mut self, s: &Settings, i: &Input) -> Option<Event> {
        let moved = i
            .cursor
            .zip(self.last)
            .is_some_and(|(a, b)| a.distance(b) > 0.5);
        let stroke = self.stroke(i);
        if i.button || i.busy {
            let event = self.wake(i.now);
            self.wake_started = None;
            self.quiet_since = None;
            return event;
        }
        if !s.sleep && matches!(self.state, State::Asleep | State::Waking) {
            return self.wake(i.now);
        }
        if matches!(self.state, State::Asleep | State::Waking) {
            if s.touch_wake && i.touching {
                self.touch_left = None;
                if moved && self.touch_since.is_none() {
                    self.touch_since = Some(i.now);
                }
                self.stirring = self.touch_since.is_some();
                if let Some(at) = self.touch_since {
                    if stroke || i.now - at >= self.touch_delay {
                        return self.wake(i.now);
                    }
                }
            } else {
                let left = *self.touch_left.get_or_insert(i.now);
                if i.now - left > 0.3 {
                    self.touch_since = None;
                    self.stirring = false;
                }
            }
            if s.audio_wake && i.audio == Audio::Playing {
                let at = *self.audio_since.get_or_insert(i.now);
                if i.now - at >= 0.5 && self.wake_at.is_none() {
                    self.state = State::Waking;
                    self.wake_started = Some(i.now);
                    self.wake_at = Some(i.now + 0.5 + self.random());
                }
            } else {
                self.audio_since = None;
            }
            if self.wake_at.is_some_and(|at| i.now >= at)
                || self.random_at.is_some_and(|at| i.now >= at)
            {
                return self.wake(i.now);
            }
            return None;
        }
        if i.nearby || i.audio != Audio::Quiet {
            self.quiet_since = None;
            self.sleep_due = false;
        } else {
            let at = *self.quiet_since.get_or_insert(i.now);
            self.sleep_due = s.sleep && i.now - at >= s.delay;
        }
        if self.sleep_due && !i.response_busy {
            self.sleep_now(i.now, s);
            return Some(Event::Sleep);
        }
        if s.petting && stroke && self.state != State::Petting && i.now >= self.pet_cooldown {
            self.state = State::Petting;
            return Some(Event::Headpat);
        }
        if self.state == State::Petting && (!s.petting || i.now - self.last_stroke > 0.8) {
            self.state = State::Awake;
            self.pet_cooldown = i.now + 3.0;
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input(now: f32) -> Input {
        Input {
            now,
            cursor: None,
            nearby: false,
            touching: false,
            pet_region: false,
            button: false,
            busy: false,
            response_busy: false,
            audio: Audio::Quiet,
            height: 146.0,
        }
    }
    #[test]
    fn sleep_pose_is_stable_per_nap_and_does_not_repeat() {
        let mut c = Controller::new(13);
        let settings = Settings {
            sleep: true,
            ..Default::default()
        };
        c.set_sleep_pose_count(3);
        let mut previous = None;
        for n in 0..30 {
            let now = n as f32 * 10.0;
            c.sleep_now(now, &settings);
            let selected = c.sleep_pose;
            assert!(selected < 3);
            assert_ne!(previous, Some(selected));
            c.step(&settings, input(now + 1.0));
            assert_eq!(c.sleep_pose, selected);
            c.wake(now + 5.0);
            previous = Some(selected);
        }
        c.set_sleep_pose_count(1);
        c.sleep_now(400.0, &settings);
        assert_eq!(c.sleep_pose, 0);
    }
    #[test]
    fn sleep_and_wake_visuals_share_timing_and_input_interrupts() {
        use super::super::motion::{Expression, Pose};
        let settings = Settings {
            sleep: true,
            ..Default::default()
        };
        let rig = super::super::rig::Rig::load();
        let mut c = Controller::new(1);
        c.sleep_now(10.0, &settings);
        let sample = |c: &Controller, now, reduced| {
            let mut pose = Pose::default();
            c.apply_sleep(&rig, &mut pose, now, reduced, false);
            pose
        };
        assert_eq!(sample(&c, 10.0, false).expression, Expression::Idle);
        assert_eq!(sample(&c, 10.8, false).expression, Expression::Blink);
        assert_eq!(sample(&c, 11.2, false).expression, Expression::Idle);
        assert_eq!(sample(&c, 12.0, false).expression, Expression::Blink);
        assert!(!c.sleep_picture(12.0));
        assert!(c.sleep_picture(12.7));
        assert!(sample(&c, 12.7, false).head_drop > 0.0);
        assert_eq!(sample(&c, 12.7, true).head_drop, 0.0);
        assert_eq!(c.wake(13.0), Some(Event::Wake));
        assert!(!c.sleep_picture(13.0));
        assert!(sample(&c, 13.1, false).head_drop > sample(&c, 14.0, false).head_drop);
        assert_eq!(sample(&c, 13.8, false).expression, Expression::Blink);
        assert!(!c.sleep_motion(14.5));
        c.sleep_now(20.0, &settings);
        let mut press = input(20.2);
        press.button = true;
        assert_eq!(c.step(&settings, press), Some(Event::Wake));
        assert!(!c.sleep_motion(20.2));
    }
    #[test]
    fn idle_response_defers_sleep_without_resetting_countdown() {
        let s = Settings {
            sleep: true,
            delay: 60.0,
            ..Settings::default()
        };
        let mut c = Controller::new(1);
        c.step(&s, input(0.0));
        let mut i = input(61.0);
        i.response_busy = true;
        assert_eq!(c.step(&s, i), None);
        assert!(c.sleep_due);
        assert_eq!(c.step(&s, input(62.0)), Some(Event::Sleep));
    }
    #[test]
    fn unavailable_audio_and_nearby_cursor_restart_countdown() {
        let s = Settings {
            sleep: true,
            delay: 60.0,
            ..Settings::default()
        };
        let mut c = Controller::new(2);
        c.step(&s, input(0.0));
        let mut i = input(59.0);
        i.audio = Audio::Unknown;
        c.step(&s, i);
        assert_eq!(c.step(&s, input(61.0)), None);
        let mut i = input(120.0);
        i.nearby = true;
        c.step(&s, i);
        assert_eq!(c.step(&s, input(121.0)), None);
        assert_eq!(c.step(&s, input(182.0)), Some(Event::Sleep));
        let mut i = input(182.1);
        i.button = true;
        assert_eq!(c.step(&s, i), Some(Event::Wake));
        assert_eq!(c.step(&s, input(183.0)), None);
    }
    #[test]
    fn rubbing_emits_once_and_buttons_cannot_pet() {
        let s = Settings {
            petting: true,
            ..Default::default()
        };
        let mut c = Controller::new(9);
        let mut events = Vec::new();
        for (n, x) in [0.0, 20.0, 40.0, 20.0, 0.0, 20.0, 40.0]
            .into_iter()
            .enumerate()
        {
            let mut i = input(n as f32 * 0.1);
            i.cursor = Some(Point::new(x, 20.0));
            i.nearby = true;
            i.touching = true;
            i.pet_region = true;
            if let Some(e) = c.step(&s, i) {
                events.push(e);
            }
        }
        assert_eq!(events, vec![Event::Headpat]);
        let mut i = input(0.8);
        i.button = true;
        i.pet_region = true;
        assert_eq!(c.step(&s, i), None);
        assert_eq!(c.state, State::Awake);
    }
    #[test]
    fn touch_stirs_then_wakes_and_audio_requires_sustained_playback() {
        let s = Settings {
            sleep: true,
            ..Default::default()
        };
        let mut c = Controller::new(11);
        c.sleep_now(0.0, &s);
        let mut i = input(0.0);
        i.cursor = Some(Point::new(0.0, 0.0));
        c.step(&s, i);
        let mut i = input(0.1);
        i.cursor = Some(Point::new(5.0, 0.0));
        i.touching = true;
        assert_eq!(c.step(&s, i), None);
        assert!(c.stirring);
        let mut i = input(3.2);
        i.cursor = Some(Point::new(5.0, 0.0));
        i.touching = true;
        assert_eq!(c.step(&s, i), Some(Event::Wake));
        c.sleep_now(4.0, &s);
        let mut i = input(4.1);
        i.audio = Audio::Playing;
        c.step(&s, i);
        c.step(&s, input(4.4));
        assert_eq!(c.state, State::Asleep);
        let mut i = input(5.0);
        i.audio = Audio::Playing;
        c.step(&s, i);
        let mut i = input(5.6);
        i.audio = Audio::Playing;
        c.step(&s, i);
        assert_eq!(c.state, State::Waking);
        assert_eq!(c.step(&s, input(7.2)), Some(Event::Wake));
    }

    #[test]
    fn affection_animates_with_bounded_hearts_and_cancels_on_pickup() {
        let settings = Settings {
            petting: true,
            ..Default::default()
        };
        let mut c = Controller::new(9);
        let rig = super::super::rig::Rig::load();
        c.pet_now(0.0);
        for n in 0..30 {
            let mut i = input(n as f32 * 0.05);
            i.cursor = Some(Point::new(200.0, 200.0));
            i.touching = true;
            i.pet_region = true;
            c.step(&settings, i);
        }
        assert!(c.affection.amount > 0.9);
        assert!(!c.affection.hearts.is_empty() && c.affection.hearts.len() <= 4);
        let base = super::super::motion::Pose {
            angles: vec![0.0; rig.motion_parts().len()],
            ..Default::default()
        };
        let mut a = base.clone();
        let mut b = base.clone();
        c.apply_affection(&rig, &mut a, 1.0, false, false);
        c.apply_affection(&rig, &mut b, 1.2, false, false);
        assert_ne!(a.angles, b.angles);
        assert!(a.angles.iter().all(|a| a.abs() < 0.2));
        let mut reduced = base.clone();
        c.apply_affection(&rig, &mut reduced, 1.2, true, false);
        assert_eq!(reduced.angles, base.angles);
        assert_eq!(reduced.head_drop, 0.0);
        let mut flat = base.clone();
        c.apply_affection(&rig, &mut flat, 1.2, false, true);
        assert!(flat.lean.abs() > 0.0 || flat.squash.abs() > 0.0);
        let mut pickup = input(1.6);
        pickup.busy = true;
        c.step(&settings, pickup);
        assert!(c.affection.hearts.is_empty());
        assert_eq!(c.affection.amount, 0.0);
    }
    #[test]
    fn gentle_touch_eases_in_and_out_without_emitting_headpat_or_hearts() {
        let settings = Settings {
            petting: true,
            ..Default::default()
        };
        let mut c = Controller::new(12);
        for n in 0..20 {
            let mut i = input(n as f32 * 0.05);
            i.touching = true;
            i.pet_region = true;
            i.cursor = Some(Point::new(200.0, 200.0));
            assert_eq!(c.step(&settings, i), None);
        }
        assert!(c.affection.amount > 0.2 && c.affection.amount <= 0.3);
        assert!(c.affection.hearts.is_empty());
        let amount = c.affection.amount;
        c.step(&settings, input(1.0));
        assert!(c.affection.amount < amount && c.affection.amount > 0.0);
        for n in 1..20 {
            c.step(&settings, input(1.0 + n as f32 * 0.05));
        }
        assert!(c.affection.amount < 0.001);
    }

    #[test]
    fn settings_round_trip_and_reject_invalid_values() {
        let s = Settings {
            sleep: true,
            petting: true,
            ..Settings::default()
        };
        assert_eq!(Settings::parse(&s.to_json()).unwrap(), s);
        assert!(Settings::parse(&json!({"delay_seconds":0})).is_err());
        assert!(Settings::parse(&json!({"mouth":[2,0]})).is_err());
    }
}

/// Sleep decoration in stage coordinates. Its bounds never participate in input.
pub fn draw_zzz(
    frame: &mut iced::widget::canvas::Frame,
    stage: iced::Size,
    settings: &Settings,
    state: State,
    stirring: bool,
    now: f32,
    reduced: bool,
) {
    if !settings.zzz || !matches!(state, State::Asleep | State::Waking) {
        return;
    }
    let origin = Point::new(
        settings.mouth[0] * stage.width,
        settings.mouth[1] * stage.height,
    );
    let alpha = if stirring || state == State::Waking {
        0.2
    } else {
        0.75
    };
    let size = (stage.height * 0.045).max(12.0);
    let direction = if origin.x > stage.width * 0.7 {
        -1.0
    } else {
        1.0
    };
    let count = if reduced { 1 } else { 3 };
    for i in 0..count {
        let t = if reduced {
            0.0
        } else {
            ((now - i as f32 * 1.2).rem_euclid(3.6)) / 2.8
        };
        if t > 1.0 {
            continue;
        }
        let x =
            (origin.x + direction * t * size * 2.0).clamp(0.0, (stage.width - size * 3.0).max(0.0));
        let y = (origin.y - t * size * 2.5).clamp(size, stage.height);
        let glyphs = if reduced { 3 } else { 1 };
        for glyph in 0..glyphs {
            let left = x + glyph as f32 * size * 0.8;
            let opacity = (alpha * (1.0 - t) * 15.0).round().clamp(0.0, 15.0) as usize;
            frame.draw_image(
                iced::Rectangle::new(Point::new(left, y), iced::Size::new(size * 0.6, size * 0.7)),
                iced::widget::canvas::Image::new(zzz_images()[opacity].clone()),
            );
        }
    }
}

fn zzz_images() -> &'static Vec<iced::advanced::image::Handle> {
    static IMAGES: std::sync::OnceLock<Vec<iced::advanced::image::Handle>> =
        std::sync::OnceLock::new();
    IMAGES.get_or_init(|| {
        (0..16)
            .map(|opacity| {
                let mut pixels = vec![0u8; 32 * 32 * 4];
                for y in 0..32usize {
                    for x in 0..32usize {
                        let ink = (y >= 2 && y <= 5 && x >= 2 && x <= 29)
                            || (y >= 26 && y <= 29 && x >= 2 && x <= 29)
                            || (y >= 4 && y <= 27 && (x as i32 + y as i32 - 31).abs() <= 2);
                        if ink {
                            let i = (y * 32 + x) * 4;
                            pixels[i..i + 4].copy_from_slice(&[
                                204,
                                217,
                                242,
                                (opacity * 17) as u8,
                            ]);
                        }
                    }
                }
                iced::advanced::image::Handle::from_rgba(32, 32, pixels)
            })
            .collect()
    })
}

#[derive(Debug, Default)]
struct Affection {
    amount: f32,
    direction: f32,
    phase: f32,
    last_time: Option<f32>,
    touch_since: Option<f32>,
    next_heart: f32,
    preview_until: f32,
    serial: u32,
    hearts: Vec<Heart>,
}
#[derive(Debug)]
struct Heart {
    born: f32,
    origin: Point,
    side: f32,
}
impl Affection {
    fn update(&mut self, i: &Input, previous: Option<Point>, active: bool, petting: bool) {
        let dt = self
            .last_time
            .map_or(0.0, |at| (i.now - at).clamp(0.0, 0.1));
        self.last_time = Some(i.now);
        if !active {
            self.amount = 0.0;
            self.touch_since = None;
            self.hearts.clear();
            return;
        }
        let touching = i.touching && i.pet_region;
        let contact = if touching {
            i.now - *self.touch_since.get_or_insert(i.now) > 0.15
        } else {
            self.touch_since = None;
            false
        };
        let target = if petting {
            1.0
        } else if contact {
            0.3
        } else {
            0.0
        };
        self.amount += (target - self.amount) * (1.0 - (-dt * 10.0).exp());
        if touching {
            if let Some((point, last)) = i.cursor.zip(previous) {
                let delta = ((point.x - last.x) / (i.height * 0.035).max(1.0)).clamp(-1.0, 1.0);
                self.direction += (delta - self.direction) * (1.0 - (-dt * 12.0).exp());
                self.phase += (point.distance(last) / (i.height * 0.08).max(1.0)).min(0.4);
            }
        } else {
            self.direction *= (-dt * 8.0).exp();
        }
        self.hearts.retain(|h| i.now - h.born < 1.5);
        if petting && (touching || i.now < self.preview_until) && i.now >= self.next_heart {
            self.serial = self.serial.wrapping_add(1);
            if self.hearts.len() < 4 {
                self.hearts.push(Heart {
                    born: i.now,
                    origin: i
                        .cursor
                        .unwrap_or(Point::new(i.height * 0.5, i.height * 0.25)),
                    side: if self.serial % 2 == 0 { -1.0 } else { 1.0 },
                });
            }
            self.next_heart = i.now + 0.45;
        }
    }
}
fn draw_heart(frame: &mut iced::widget::canvas::Frame, origin: Point, size: f32, alpha: f32) {
    let opacity = (alpha * 15.0).round().clamp(0.0, 15.0) as usize;
    frame.draw_image(
        iced::Rectangle::new(origin, iced::Size::new(size, size)),
        iced::widget::canvas::Image::new(heart_images()[opacity].clone()),
    );
}
fn heart_images() -> &'static Vec<iced::advanced::image::Handle> {
    static IMAGES: std::sync::OnceLock<Vec<iced::advanced::image::Handle>> =
        std::sync::OnceLock::new();
    IMAGES.get_or_init(|| {
        let mask: Vec<u8> = (0..48 * 48)
            .map(|pixel| {
                let mut coverage = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let x = ((pixel % 48) as f32 + (sx as f32 + 0.5) / 4.0) / 48.0 * 2.8 - 1.4;
                        let y = 1.3 - ((pixel / 48) as f32 + (sy as f32 + 0.5) / 4.0) / 48.0 * 2.8;
                        let a = x * x + y * y - 1.0;
                        if a * a * a - x * x * y * y * y <= 0.0 {
                            coverage += 1;
                        }
                    }
                }
                (coverage * 255 / 16) as u8
            })
            .collect();
        (0..16)
            .map(|opacity| {
                let pixels: Vec<u8> = mask
                    .iter()
                    .flat_map(|a| [255, 116, 159, (*a as u32 * opacity / 15) as u8])
                    .collect();
                iced::advanced::image::Handle::from_rgba(48, 48, pixels)
            })
            .collect()
    })
}
