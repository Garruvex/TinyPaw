use super::reactions::{Art, Event};
use iced::{Point, Size};

#[derive(Debug, Clone)]
pub enum Action {
    Open(Event),
    Opacity(f32),
    Compare(bool),
    Zoom(f32),
    Match,
    Point(Point),
    Place([f32; 3]),
    Reset,
    Finish(bool),
}

pub struct Alignment {
    pub event: Event,
    pub art: Art,
    pub opacity: f32,
    pub side_by_side: bool,
    pub zoom: f32,
    pub matching: bool,
    pub points: Vec<Point>,
    pub error: Option<String>,
}

impl Alignment {
    pub fn new(event: Event, art: Art) -> Self {
        Self {
            event,
            art,
            opacity: 0.5,
            side_by_side: false,
            zoom: 1.0,
            matching: false,
            points: Vec::new(),
            error: None,
        }
    }

    pub fn point(&mut self, point: Point, stage: Size) {
        self.points.push(point);
        if self.points.len() < 4 {
            return;
        }
        let result = match_landmarks(self.art.place, &self.points, stage);
        match result {
            Ok(place) => self.art.place = place,
            Err(error) => self.error = Some(error.into()),
        }
        self.matching = false;
        self.points.clear();
    }

    pub fn instruction(&self) -> &'static str {
        match self.points.len() {
            0 => "Click the left cheek on the resting character.",
            1 => "Click the right cheek on the resting character.",
            2 => "Click the same left cheek on the replacement pose.",
            _ => "Click the same right cheek on the replacement pose.",
        }
    }
}

fn match_landmarks(
    place: [f32; 3],
    points: &[Point],
    stage: Size,
) -> Result<[f32; 3], &'static str> {
    let resting = points[0].distance(points[1]);
    let posed = points[2].distance(points[3]);
    if resting < 2.0 || posed < 2.0 {
        return Err("Choose two distinct landmarks farther apart.");
    }
    let ratio = resting / posed;
    let target = points[0] + (points[1] - points[0]) * 0.5;
    let current = points[2] + (points[3] - points[2]) * 0.5;
    let result = [
        (target.x - (current.x - place[0] * stage.width) * ratio) / stage.width,
        (target.y - (current.y - place[1] * stage.height) * ratio) / stage.height,
        place[2] * ratio,
    ];
    if result.iter().any(|v| !v.is_finite())
        || result[0].abs() > 1.0
        || result[1].abs() > 1.0
        || !(0.1..=4.0).contains(&result[2])
    {
        return Err(
            "This match exceeds the supported placement range. Adjust the pose size or choose other landmarks.",
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_scales_and_aligns_midpoints_without_changing_head_proportions() {
        let points = [
            Point::new(40.0, 30.0),
            Point::new(80.0, 30.0),
            Point::new(20.0, 50.0),
            Point::new(40.0, 50.0),
        ];
        assert_eq!(
            match_landmarks([0.0, 0.0, 1.0], &points, Size::new(100.0, 100.0)).unwrap(),
            [0.0, -0.7, 2.0]
        );
        let mut degenerate = points;
        degenerate[3] = degenerate[2];
        assert!(match_landmarks([0.0, 0.0, 1.0], &degenerate, Size::new(100.0, 100.0)).is_err());
    }
}
