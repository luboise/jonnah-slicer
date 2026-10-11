macro_rules! fill {
    ($ui:ident, $opacity:literal * $colour:expr) => {
        crate::draw::fill!($ui, $colour, $opacity)
    };

    ($ui:ident, $opacity:ident * $colour:expr) => {
        crate::draw::fill!($ui, $colour, $opacity)
    };

    ($ui:ident, $colour:expr, $opacity:expr) => {
        $crate::draw::fill_ui_transparent($ui, $colour, $opacity)
    };

    ($ui:ident, $colour:expr) => {
        $crate::draw::fill_ui($ui, $colour)
    };
}

pub(crate) use fill;

#[inline]
pub fn fill_ui(ui: &mut egui::Ui, colour: egui::Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0, colour);
}

#[inline]
pub fn fill_ui_transparent(ui: &mut egui::Ui, colour: egui::Color32, opacity: f32) {
    fill_ui(ui, colour.gamma_multiply(opacity));
}
