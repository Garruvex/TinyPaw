//! What the characters on the stage share so they behave as neighbours: where
//! each one stands, and whose turn it is to speak. Each stage thread keeps its
//! own character; this is the one small piece of state between them.
//!
//! - **Spatial:** a character coming to rest asks for the nearest free spot
//!   along the ground, so two never stand on each other.
//! - **Turn-taking:** one bubble at a time. A line nobody asked for waits its
//!   turn; one the user provoked (a click, a drop) takes the turn.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Gap kept between two characters, in physical pixels.
const GAP: f32 = 6.0;

/// Where one character stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub id: u32,
    /// Feet, physical desktop pixels.
    pub x: f32,
    pub y: f32,
    /// Half the width it occupies on the ground.
    pub half: f32,
}

#[derive(Default)]
struct Inner {
    slots: Vec<Slot>,
    speaker: Option<(u32, Instant)>,
}

#[derive(Default)]
pub struct Crowd {
    inner: Mutex<Inner>,
}

impl Crowd {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records where a character stands now.
    pub fn publish(&self, slot: Slot) {
        let mut inner = self.lock();
        match inner.slots.iter_mut().find(|s| s.id == slot.id) {
            Some(existing) => *existing = slot,
            None => inner.slots.push(slot),
        }
    }

    /// A character left the stage.
    pub fn remove(&self, id: u32) {
        let mut inner = self.lock();
        inner.slots.retain(|s| s.id != id);
        if inner.speaker.is_some_and(|(speaker, _)| speaker == id) {
            inner.speaker = None;
        }
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.lock().slots.len()
    }

    /// The nearest `x` to the wanted one, within `lo..=hi`, where a character
    /// of this width does not overlap another standing at about the same
    /// height. If the ground is full it keeps the wanted spot.
    pub fn clear_x(&self, id: u32, x: f32, y: f32, half: f32, lo: f32, hi: f32) -> f32 {
        let inner = self.lock();
        let others: Vec<&Slot> = inner
            .slots
            .iter()
            .filter(|s| s.id != id && (s.y - y).abs() < half.max(s.half))
            .collect();
        let overlaps = |at: f32| {
            others
                .iter()
                .any(|o| (at - o.x).abs() < half + o.half + GAP - 0.5)
        };
        let wanted = x.clamp(lo, hi.max(lo));
        if !overlaps(wanted) {
            return wanted;
        }
        let mut best: Option<f32> = None;
        for o in &others {
            for side in [-1.0_f32, 1.0] {
                let at = o.x + side * (o.half + half + GAP);
                if at < lo || at > hi || overlaps(at) {
                    continue;
                }
                if best.is_none_or(|b| (at - wanted).abs() < (b - wanted).abs()) {
                    best = Some(at);
                }
            }
        }
        best.unwrap_or(wanted)
    }

    /// Asks for the turn to speak for `seconds`. A line nobody asked for gets
    /// it only when nobody else is speaking; a prompted one always does. The
    /// current speaker may extend its own turn.
    pub fn claim_voice(&self, id: u32, now: Instant, seconds: f32, prompted: bool) -> bool {
        let mut inner = self.lock();
        let busy = inner
            .speaker
            .is_some_and(|(who, until)| who != id && until > now);
        if busy && !prompted {
            return false;
        }
        let until = now + Duration::from_secs_f32(seconds.max(0.0));
        let until = match inner.speaker {
            Some((who, current)) if who == id && current > until => current,
            _ => until,
        };
        inner.speaker = Some((id, until));
        true
    }

    /// Whether this character still has the turn. A character that lost it
    /// to a prompted line puts its own bubble away.
    pub fn has_voice(&self, id: u32, now: Instant) -> bool {
        self.lock()
            .speaker
            .is_none_or(|(who, until)| who == id || until <= now)
    }

    /// The character is done speaking.
    pub fn release_voice(&self, id: u32) {
        let mut inner = self.lock();
        if inner.speaker.is_some_and(|(who, _)| who == id) {
            inner.speaker = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: u32, x: f32) -> Slot {
        Slot {
            id,
            x,
            y: 900.0,
            half: 50.0,
        }
    }

    #[test]
    fn a_character_resting_on_another_moves_to_the_nearest_free_spot() {
        let crowd = Crowd::default();
        crowd.publish(slot(1, 500.0));
        // Free ground is left alone.
        assert_eq!(crowd.clear_x(2, 800.0, 900.0, 50.0, 0.0, 1920.0), 800.0);
        // On top of the first: shifted just clear of it, on the nearer side.
        let at = crowd.clear_x(2, 520.0, 900.0, 50.0, 0.0, 1920.0);
        assert_eq!(at, 500.0 + 50.0 + 50.0 + GAP);
        let at = crowd.clear_x(2, 480.0, 900.0, 50.0, 0.0, 1920.0);
        assert_eq!(at, 500.0 - 50.0 - 50.0 - GAP);
    }

    #[test]
    fn a_character_at_another_height_is_not_in_the_way_and_the_screen_edge_is_respected() {
        let crowd = Crowd::default();
        crowd.publish(Slot {
            y: 300.0,
            ..slot(1, 500.0)
        });
        assert_eq!(crowd.clear_x(2, 500.0, 900.0, 50.0, 0.0, 1920.0), 500.0);
        let crowd = Crowd::default();
        crowd.publish(slot(1, 60.0));
        // The left side would be off screen, so it goes right.
        let at = crowd.clear_x(2, 70.0, 900.0, 50.0, 50.0, 1870.0);
        assert_eq!(at, 60.0 + 100.0 + GAP);
    }

    #[test]
    fn many_characters_each_find_a_place_without_overlapping() {
        let crowd = Crowd::default();
        for id in 1..=12 {
            let at = crowd.clear_x(id, 960.0, 900.0, 50.0, 50.0, 1870.0);
            crowd.publish(slot(id, at));
        }
        let mut xs: Vec<f32> = (1..=12)
            .map(|id| {
                let inner = crowd.lock();
                inner.slots.iter().find(|s| s.id == id).unwrap().x
            })
            .collect();
        xs.sort_by(f32::total_cmp);
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= 100.0 + GAP - 0.5, "{xs:?}");
        }
        crowd.remove(3);
        assert_eq!(crowd.count(), 11);
    }

    #[test]
    fn one_bubble_at_a_time_and_a_prompted_line_takes_the_turn() {
        let crowd = Crowd::default();
        let t0 = Instant::now();
        assert!(crowd.claim_voice(1, t0, 3.0, false));
        // Another unprompted line waits; the speaker may extend its own turn.
        assert!(!crowd.claim_voice(2, t0, 3.0, false));
        assert!(crowd.claim_voice(1, t0, 5.0, false));
        assert!(crowd.has_voice(1, t0) && !crowd.has_voice(2, t0));
        // The user clicking character 2 takes the turn; 1 must stop.
        assert!(crowd.claim_voice(2, t0, 2.0, true));
        assert!(!crowd.has_voice(1, t0) && crowd.has_voice(2, t0));
        // After the turn ends anyone may speak.
        let later = t0 + Duration::from_secs(3);
        assert!(crowd.has_voice(1, later));
        assert!(crowd.claim_voice(1, later, 1.0, false));
        crowd.release_voice(1);
        assert!(crowd.claim_voice(2, later, 1.0, false));
    }
}
