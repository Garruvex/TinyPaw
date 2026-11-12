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
        Ok(s)
    }
    pub fn to_json(&self) -> Value {
        json!({"petting":self.petting,"sleep":self.sleep,"delay_seconds":self.delay,
            "touch_wake":self.touch_wake,"audio_wake":self.audio_wake,
            "random_wake":self.random_wake,"zzz":self.zzz,"mouth":self.mouth})
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
        self.state = State::Petting;
        self.last_stroke = now;
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.seed);
    }
    pub fn wake(&mut self, now: f32) -> Option<Event> {
        let was_sleeping = matches!(self.state, State::Asleep | State::Waking);
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
    pub fn sleep_now(&mut self, now: f32, s: &Settings) {
        self.state = State::Asleep;
        self.touch_delay = 1.0 + 2.0 * self.random();
        self.random_at = s.random_wake.then(|| now + 300.0 + 600.0 * self.random());
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
        let valid = self.travel >= 40.0 * scale && self.reversals >= 2;
        if valid {
            self.last_stroke = i.now;
        }
        valid
    }
    pub fn step(&mut self, s: &Settings, i: Input) -> Option<Event> {
        let moved = i
            .cursor
            .zip(self.last)
            .is_some_and(|(a, b)| a.distance(b) > 0.5);
        let stroke = self.stroke(&i);
        if i.button || i.busy {
            let event = self.wake(i.now);
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
        frame.fill_text(iced::widget::canvas::Text {
            content: if reduced { "Zzz" } else { "z" }.into(),
            position: Point::new(x, y),
            color: iced::Color::from_rgba(0.65, 0.70, 0.82, alpha * (1.0 - t)),
            size: size.into(),
            ..Default::default()
        });
    }
}
