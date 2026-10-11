use crate::audio::RatioExt as _;

pub const NOTE_HEIGHT: f32 = 12.0;

pub enum NoteType {
    Scratch,
    White,
    Black,
}

impl NoteType {}

pub struct Note {
    pub tp: crate::audio::TimePoint,
}

pub const fn column_colour(i: usize) -> egui::Color32 {
    let grayness = if i == 0 {
        // scratch
        9
    } else if i.is_multiple_of(2) {
        // black
        9
    } else {
        // white
        28
    };

    egui::Color32::from_gray(grayness)
}

#[must_use]
pub fn draw_column(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    colour: Option<egui::Color32>,
) -> egui::Rect {
    let (col_rect, _) =
        ui.allocate_exact_size(egui::Vec2::new(width, height), egui::Sense::hover());

    if let Some(colour) = colour {
        ui.painter().rect_filled(col_rect, 0, colour);
    }

    col_rect
}

/// Draw notes in the rect of a column
pub fn draw_notes_in_column(
    ui: &mut egui::Ui,
    col_rect: egui::Rect,
    range: std::ops::Range<crate::audio::TimePoint>,
    colour: egui::Color32,
    notes: &[Note],
) {
    let painter = ui.painter();

    let draw_note = move |height| {
        let egui::emath::Rangef { min, max } = col_rect.y_range();

        let bottom = max - (max - min) * height as f32;

        let tl = egui::Pos2 {
            x: col_rect.min.x,
            y: bottom - NOTE_HEIGHT,
        };

        let size = egui::Vec2 {
            x: col_rect.width(),
            y: NOTE_HEIGHT,
        };

        let note_rect = egui::Rect::from_min_size(tl, size);

        painter.rect_filled(note_rect, col_rect.width() * 0.05, colour);
    };

    for note in notes {
        let note_ratio = (note.tp - range.start) / (range.end - range.start);
        draw_note(note_ratio.to_f64());
    }
}
