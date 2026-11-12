use super::*;
use canvas::Program;

#[test]
fn drag_stays_captured_across_transparent_space_and_zoom_preserves_canvas_units() {
    use crate::mascot::editor::{Editor, Selected};
    use iced::{Point, Rectangle, Size, mouse};
    let dir = std::env::temp_dir().join(format!("tinypaw-arrange-input-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("body.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([100, 140, 220, 255]))
        .save(&file)
        .unwrap();
    let mut editor = Editor::from_body(&file, &[]).unwrap();
    editor.select(Selected::Body);
    let bounds = Rectangle::new(Point::new(30.0, 40.0), Size::new(300.0, 340.0));
    for zoom in [1.0, 1.5, 2.0] {
        editor.zoom = zoom;
        let preview = ArtPreview {
            editor: &editor,
            busy: false,
        };
        let r = preview.image_rect(bounds.size());
        let start = Point::new(bounds.x + r.center_x(), bounds.y + r.center_y());
        let mut state = ArtPreviewState::default();
        let press = preview
            .update(
                &mut state,
                &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(start),
            )
            .unwrap()
            .into_inner();
        assert!(matches!(
            press.0,
            Some(Message::Ui(UiMsg::ArtDragStart(Selected::Body)))
        ));
        let moved = Point::new(start.x + 10.0 * r.width / editor.canvas.0 as f32, start.y);
        let action = preview
            .update(
                &mut state,
                &iced::Event::Mouse(mouse::Event::CursorMoved { position: moved }),
                bounds,
                mouse::Cursor::Available(moved),
            )
            .unwrap()
            .into_inner();
        let Some(Message::Ui(UiMsg::ArtDrag(dx, dy))) = action.0 else {
            panic!("expected drag")
        };
        assert!((dx - 10.0).abs() < 0.001 && dy.abs() < 0.001);
        assert_eq!(action.2, iced::event::Status::Captured);
        let outside = Point::new(bounds.x + bounds.width + 50.0, bounds.y);
        let release = preview
            .update(
                &mut state,
                &iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(outside),
            )
            .unwrap()
            .into_inner();
        assert_eq!(release.2, iced::event::Status::Captured);
        assert!(state.drag.is_none());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
