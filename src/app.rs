use std::hash::{Hash as _, Hasher as _};

use egui::{Button, emath::Numeric as _};

use audio::RatioExt;
use log::warn;

use crate::audio::{self, calculate_num_samples};

pub const STEM_HEIGHT: f32 = 200.0;

#[derive(Default)]
struct InputState {
    pub mouse_pos: Option<egui::Pos2>,
    pub lmb_down: bool,
    pub rmb_down: bool,
    pub scroll_delta: egui::Vec2,
    pub shift_pressed: bool,
    pub zoom_delta: egui::Vec2,
    pub home_pressed: bool,
    pub g_pressed: bool,
    pub l_pressed: bool,
    pub f5_pressed: bool,
    pub number_pressed: Option<u8>,
}

impl InputState {
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        ctx.input_mut(|i| {
            let first_number_pressed = (0..9).find_map(|num| {
                i.consume_key(
                    egui::Modifiers::NONE,
                    egui::Key::from_name(&format!("{}", (b'0' + num) as char))?,
                )
                .then_some(num)
            });
            Self {
                mouse_pos: i.pointer.latest_pos(),
                lmb_down: i.pointer.button_down(egui::PointerButton::Primary),
                rmb_down: i.pointer.button_down(egui::PointerButton::Secondary),
                scroll_delta: i.smooth_scroll_delta(),
                shift_pressed: i.modifiers.shift,
                zoom_delta: i.zoom_delta_2d(),
                home_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::Home),
                g_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::G),
                l_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::L),
                f5_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::F5),
                number_pressed: first_number_pressed,
            }
        })
    }
}

#[derive(Default, Debug)]
enum ProjectStatus {
    #[default]
    None,
    Loaded,
    Failed(crate::Error),
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct JonnahSlicer<'a> {
    project_path: Option<std::path::PathBuf>,
    #[serde(skip)]
    project: LiveProject,

    display_start: crate::audio::TimePoint,

    /// The number of points to draw in channel
    visual_density: usize,

    slice_snapping: crate::audio::Snapping,

    zoom_level: f32,
    audio_volume: f32,
    group_colour_opacity: f32,

    #[serde(skip)]
    quit_application: bool,

    // whether the project should be refreshed at the start of the next frame
    #[serde(skip)]
    refresh_project: bool,

    #[serde(skip)]
    previous_diagnostic: crate::logging::LogState,

    // slices gathered from dropped midi files
    #[serde(skip)]
    midi_file_slices: Vec<crate::project::Slice>,

    #[serde(skip)]
    input_state: InputState,

    #[serde(skip)]
    project_status: ProjectStatus,

    #[serde(skip)]
    jonnah_image: Option<egui::Image<'a>>,
    #[serde(skip)]
    drag_and_drop: egui::DragAndDrop,
    #[serde(skip)]
    audio_player: Option<crate::audio_player::AudioPlayer>,
}

#[derive(Debug)]
struct LiveStem {
    stem: crate::project::Stem,
    audio: Option<crate::audio::AudioFile>,
    locked: bool,
}

impl From<crate::project::Stem> for LiveStem {
    fn from(stem: crate::project::Stem) -> Self {
        Self {
            stem,
            audio: None,
            locked: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct LiveProject {
    sample_rate: crate::project::SampleRate,
    stems: Vec<LiveStem>,
    timing: audio::Timing,
}

impl std::convert::TryFrom<crate::project::Project> for LiveProject {
    type Error = crate::Error;

    fn try_from(project: crate::project::Project) -> Result<Self, Self::Error> {
        let crate::project::Project {
            sample_rate,
            stems,
            timing,
        } = project;

        Ok(Self {
            sample_rate,
            stems: stems.into_iter().map(|v| v.into()).collect(),
            timing,
        })
    }
}

impl LiveProject {
    pub fn as_project(&self) -> crate::project::Project {
        let Self {
            sample_rate,
            stems,
            timing,
        } = self;

        crate::project::Project {
            sample_rate: *sample_rate,
            stems: stems.iter().map(|stem| stem.stem.clone()).collect(),
            timing: timing.clone(),
        }
    }
}

impl Default for JonnahSlicer<'_> {
    fn default() -> Self {
        Self {
            // Example stuff:
            project: LiveProject::default(),
            visual_density: 6000,
            audio_volume: 1.0,
            input_state: InputState::default(),
            zoom_level: 1.0,
            jonnah_image: None,
            display_start: crate::audio::TimePoint::default(),
            slice_snapping: crate::audio::Snapping::default(),
            audio_player: match crate::audio_player::AudioPlayer::new(
                crate::project::SampleRate::default(),
            ) {
                Ok(v) => {
                    log::info!(
                        "initialised audio player @ {}hz",
                        crate::project::SampleRate::default().0,
                    );
                    Some(v)
                }
                Err(e) => {
                    log::error!("failed to initialise audio engine: {e}");
                    None
                }
            },
            project_path: None,
            drag_and_drop: egui::DragAndDrop::default(),
            project_status: Default::default(),
            midi_file_slices: Default::default(),
            group_colour_opacity: 0.5,
            refresh_project: true,
            previous_diagnostic: Default::default(),
            quit_application: false,
        }
    }
}

impl JonnahSlicer<'_> {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, logging_mutex: crate::logging::LogState) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app = if let Some(storage) = cc.storage {
            let mut x: JonnahSlicer<'_> =
                eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();

            if x.project_path.is_none() {
                x.project_path = Some(std::path::PathBuf::from("./projects/new_project/"));
            }

            x
        } else {
            Default::default()
        };

        app.audio_player = crate::audio_player::AudioPlayer::new(app.project.sample_rate).ok();
        app.audio_player.as_ref().inspect(|player| {
            player.set_volume(app.audio_volume);
        });

        app.previous_diagnostic = logging_mutex;

        app
    }

    // TODO: Document errors that this function can return
    pub fn save_to_disk(&mut self) -> Result<(), crate::Error> {
        crate::project::save_project(
            &self.project.as_project(),
            self.project_path
                .clone()
                .map(crate::project::normalise_project_path)
                .unwrap_or_else(|| std::path::PathBuf::from("./project.jonnah")),
        )
    }

    pub fn draw_everything(&mut self, ui: &mut egui::Ui) {}

    pub fn default_export_dir(&self) -> std::path::PathBuf {
        self.project_path
            .as_ref()
            .map(|v| v.join("out"))
            .unwrap_or_else(|| "./".into())
    }
}

impl eframe::App for JonnahSlicer<'_> {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.refresh_project {
            use std::cmp::Ordering::*;

            self.refresh_project = false;

            self.project.stems.sort_by(|stem1, stem2| {
                let g1 = &stem1.stem.group;
                let g2 = &stem2.stem.group;

                if g1.is_some() && g2.is_none() {
                    Less
                } else if g1.is_none() && g2.is_some() {
                    Greater
                } else {
                    g1.cmp(g2).reverse()
                }
            });
        }

        self.input_state = InputState::from_ctx(ctx);
        if let Some(project_path) = &self.project_path {
            match &self.project_status {
                ProjectStatus::None => {
                    self.project = crate::project::load_project(project_path)
                        .and_then(|project| project.try_into())
                        .unwrap_or_default();

                    if self.audio_player.is_some() {
                        let new_audio_player =
                            crate::audio_player::AudioPlayer::new(self.project.sample_rate).ok();
                        self.audio_player =
                            new_audio_player.inspect(|player| player.set_volume(self.audio_volume));
                    }

                    self.project_status = ProjectStatus::Loaded;
                }
                ProjectStatus::Failed(_error) => (),
                ProjectStatus::Loaded => (),
            }
        }
        if ctx.input_mut(|ui| {
            ui.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::S,
            ))
        }) {
            self.save_to_disk();
        }

        if self.jonnah_image.is_none() {
            egui_extras::install_image_loaders(ctx);
            self.jonnah_image = Some(
                egui::Image::new(egui::include_image!("../assets/jonnah.jpg"))
                    .corner_radius(5.0)
                    .tint(egui::Color32::WHITE),
            );
        }

        let (dropped, hovered) =
            ctx.input(|i| (i.raw.dropped_files.clone(), i.raw.hovered_files.clone()));

        if !hovered.is_empty() {
            egui::Panel::left("File Hover Preview").show(ctx, |ui| {
                if !hovered.is_empty() {
                    for path in hovered.into_iter().filter_map(|file| file.path) {
                        let hovered_file = path.display().to_string();
                    }
                }
            });
        }

        for dropped_file in dropped {
            if let Some(path) = dropped_file.path {
                let parent_dir = std::env::current_dir();

                let file_path = parent_dir
                    .ok()
                    .and_then(|parent| pathdiff::diff_paths(&path, parent))
                    .unwrap_or_else(|| path.canonicalize().unwrap_or(path));

                let Some(extension) = file_path.extension() else {
                    log::error!(
                        "file {} has no extension. skipping this file",
                        file_path.display()
                    );
                    continue;
                };

                if extension == "wav" {
                    self.project.stems.push(LiveStem {
                        stem: crate::project::Stem::from_audio_path(file_path),
                        audio: None,
                        locked: false,
                    });
                } else if extension == "mid" || extension == "midi" {
                    let Ok(bytes) = std::fs::read(&file_path) else {
                        log::error!("failed to read file {}", file_path.display());
                        continue;
                    };

                    let use_ableton_midi = true;

                    let timing = if use_ableton_midi {
                        // ableton midi clips are at 120bpm and ignore bpm changes afaik
                        &crate::audio::Timing::default()
                    } else {
                        &self.project.timing
                    };

                    let Ok(midi) = crate::slices_from_midi(&bytes, timing) else {
                        log::error!("failed to parse midi file {}", file_path.display());
                        continue;
                    };

                    self.midi_file_slices.extend(midi);
                }
            }
        }

        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        // TODO: Make this periodic
        self.project.stems.iter_mut().for_each(|st| {
            if st.audio.is_none()
                && let Ok(wav) = wavers::Wav::from_path(&st.stem.audio_path)
            {
                // TODO: FIX THIS
                st.audio = Some(crate::audio::AudioFile::new(wav).expect("Bad wav"));
            }
        });

        if self.input_state.home_pressed {
            self.display_start = Default::default();
        }

        if self.input_state.zoom_delta.y != 1.0 {
            self.zoom_level *= 1.0 + (0.4 * (self.input_state.zoom_delta.y - 1.0));
        }

        if self.input_state.scroll_delta.x != 0.0
            || (self.input_state.shift_pressed && self.input_state.scroll_delta.y != 0.0)
        {
            let vertical_scroll_sensitivity = num_rational::Ratio::new(1, 67);
            let horizontal_scroll_sensitivity = num_rational::Ratio::new(1, 67);

            let x = self.input_state.scroll_delta.x as i64;
            let y = self.input_state.scroll_delta.y as i64;

            let vertical = num_rational::Ratio::from_integer(y) * vertical_scroll_sensitivity;

            let horizontal = horizontal_scroll_sensitivity * num_rational::Ratio::from_integer(x);

            let diff = num_rational::Ratio::new((self.zoom_level * 1000.0) as i64, 1000)
                * (horizontal
                    + if self.input_state.shift_pressed {
                        vertical
                    } else {
                        0.into()
                    });

            self.display_start =
                (self.display_start + audio::TimePoint::from(-diff)).clamped_to_zero();
        }


        if self.quit_application {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        /*
        egui::Panel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Save").clicked() {
                            self.save_to_disk();
                        }

                        if ui.button("Quit").clicked() {
                            self.quit_application = true;
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        */

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_everything(ui);
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_panel").show_inside(ui, |ui|{
            ui.horizontal_top(|ui|{

                ui.menu_button("File", |ui| {
                    if ui.button("Save").clicked() {
                        if let Err(e) = self.save_to_disk() {
                            log::error!("failed to save project: {e}");
                        }
                    }

                    if ui.button("Quit").clicked() {
                        self.quit_application = true;
                    }
                });
                ui.add_space(16.0);
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.label("Snapping: ");
            const SNAPPINGS: [u16; 12] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, 128];

            for snap_v in SNAPPINGS {
                let button = ui.button(format!("1/{snap_v}"));

                if self.slice_snapping.as_measure_denom() == snap_v {
                    button.highlight();
                } else {
                    if let Some(num) = self.input_state.number_pressed {
                        //    [1, 2, 3, ..., 9, 0]
                        // => [0, 1, 2, 3, ..., 8, 9]
                        let num = (num + 9) % 10;

                        self.slice_snapping =
                            crate::audio::Snapping::Measure(SNAPPINGS[num as usize]);
                    } else if button.clicked() {
                        self.slice_snapping = crate::audio::Snapping::Measure(snap_v);
                    }
                }
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(jonnah) = &self.jonnah_image {
                jonnah.paint_at(ui, ui.content_rect());
            }

            ui.vertical(|ui| {
                ui.horizontal(|ui|{
                    ui.heading(format!("JonnahSlicer v{}", env!("CARGO_PKG_VERSION")));

                    let mut diagnostic = self.previous_diagnostic.lock().unwrap();

                    if diagnostic.as_ref().is_some_and(|(time, _)| {
                         std::time::SystemTime::now().duration_since(*time).unwrap_or(std::time::Duration::from_secs(10))
                             >= std::time::Duration::from_secs(10) 
                    }) {
                        *diagnostic = None;
                    } 

                    diagnostic.as_ref().inspect(|(_, msg)|{
                        ui.label(msg);
                    });
                });

                ui.horizontal(|ui| {
                    if ui.button("⟳").on_hover_text("Refresh the project, ordering stems by group (Key: F5)").clicked() || self.input_state.f5_pressed {
                        self.refresh_project = true;
                    }

                    // The central panel the region left after adding TopPanel's and SidePanel's
                    ui.add(egui::Slider::new(&mut self.zoom_level, 0.0..=8.0).text("Zoom"));
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.project.sample_rate,
                                crate::project::SampleRate::MIN..=crate::project::SampleRate::MAX,
                            )
                            .text("Sample Rate"),
                        )
                        .changed()
                    {
                        let new_audio_player =
                            match crate::audio_player::AudioPlayer::new(self.project.sample_rate) {
                                Ok(audio_player) => {
                                    log::info!(
                                        "updated audio player sample rate to {}",
                                        self.project.sample_rate.0
                                    );
                                    Some(audio_player)
                                }
                                Err(e) => {
                                    log::error!(
                                        "failed to set audio player sample rate to {}: {e}",
                                        self.project.sample_rate.0
                                    );
                                    None
                                }
                            };

                        self.audio_player =
                            new_audio_player.inspect(|player| player.set_volume(self.audio_volume));
                    }

                    ui.add_space(100.0);

                    ui.add(egui::Slider::new(&mut self.group_colour_opacity, 0.0..=1.0).max_decimals(2).text("Group Opacity"));

                    if let Some(audio_player) = &mut self.audio_player {
                        if ui
                            .add(
                                egui::Slider::new(&mut self.audio_volume, 0.0..=1.0).text("Volume"),
                            )
                            .changed()
                        {
                            audio_player.set_volume(self.audio_volume);
                        }
                    } else {
                        ui.colored_label(egui::Color32::RED, "audio player not initialised");
                    }

                    if ui.button("Export All Stems").clicked() {
                        let mut exported = None;

                        for stem in self.project.stems.iter().filter(|stem| !stem.stem.slices.0.is_empty()) {
                            let wrap_export_stem = || { 
                                let audio = stem.audio.as_ref().ok_or("no stem")?;
                                let stem_prefix = stem.stem.audio_path.file_stem()
                                    .and_then(|v| v.to_str())
                                    .ok_or("bad stem prefix")?;

                                let slices = &stem.stem.slices;
                                audio::export_stem(self.default_export_dir(),stem_prefix, &[audio], slices, &self.project.timing)
                            };

                            if let Err(e) = wrap_export_stem() {
                                log::error!("failed to export all stems: {e}");
                                exported = None;
                                break;
                            }

                            *exported.get_or_insert(0usize) += 1;
                        }

                        if let Some(exported) = exported {
                            log::info!("exported {exported} stems");
                        }
                    }

                    if ui.button("Generate BMS File").clicked() {
                        if let Err(e) = export_bms_file(self.default_export_dir().join("out.bms"), &self.project.as_project()) {
                            log::error!("failed to export BMS file: {e}");
                        }
                    }
                });
            });

            ui.separator();

            let display_length = num_rational::Ratio::new(8 * (256.0 * self.zoom_level).round() as i64, 256);

            egui::ScrollArea::vertical()
                .scroll_source(if self.input_state.shift_pressed {
                    egui::scroll_area::ScrollSource::NONE
                } else {
                    egui::scroll_area::ScrollSource::MOUSE_WHEEL
                })
                .show(ui, |ui| {
                    let end_time_point =
                        self.display_start + display_length.trunc();

                    // Draw measures labels
                    {
                        let (rect, _response) = ui.allocate_exact_size(
                            egui::Vec2::new(ui.available_width(), 20.0),
                            egui::Sense::click(),
                        );

                        let start = self.display_start.to_f64();
                        let end = self.display_start.to_integer() as f64 + display_length.to_f64();

                        let mut i = self.display_start.ceil().to_f64();
                        while i < end {
                            let tx = (i - start) / (end - start);
                            let pos = egui::pos2(rect.min.x + tx as f32 * rect.width(), rect.min.y);

                            ui.put(
                                egui::Rect::from_pos(pos).expand(20.0),
                                egui::Label::new(i.to_string()),
                            );

                            i += 1.0;
                        }
                    }

                    let export_dir = self.default_export_dir();

                    let mut stem_to_export = None;
                    let mut stem_to_delete = None;

                    for (stem_i, stem) in  self.project.stems.iter_mut().enumerate() {
                        let full_stem_dims = [ui.available_width(), STEM_HEIGHT];

                        ui.allocate_ui_with_layout(
                        full_stem_dims.into(),
                        egui::Layout::left_to_right(egui::Align::Max),
                        |ui| {
                            ui.allocate_ui_with_layout(
                                [(ui.available_width() * 0.4).min(200.0), STEM_HEIGHT].into(),
                                egui::Layout::top_down_justified(egui::Align::Center),
                                |ui| {
                                    if ui.button("Export").clicked() {
                                        stem_to_export = Some(stem_i);
                                    }

                                    ui.horizontal(|ui| {
                                        ui.label("Start: ");

                                        let button = if stem.stem.starting_keysound.is_none() {
                                            ui.button("Auto")
                                        } else {
                                            ui.button("Fixed")
                                        };

                                        if button.clicked() {
                                            stem.stem.starting_keysound =
                                                match stem.stem.starting_keysound {
                                                    Some(_) => None,
                                                    None => Some(1),
                                                };
                                        }
                                        if let Some(starting_keysound) =
                                            &mut stem.stem.starting_keysound
                                        {
                                            ui.add(
                                                egui::DragValue::new(starting_keysound)
                                                    .speed(1.0)
                                                    .custom_formatter(|v, _range| {
                                                        format!(
                                                            "{:0>2}",
                                                            base62::encode(u128::from(v as u64))
                                                        )
                                                    }),
                                            );
                                        }
                                    });


                                    {
                                        use crate::project::StemType;

                                    let button_text = match stem.stem.ty {
                                        StemType::Note => "Note",
                                        StemType::BGM => "BGM"
                                    };

                                    let button_colour = match stem.stem.ty {
                                        StemType::Note => egui::Color32::BLUE,
                                        StemType::BGM => egui::Color32::RED
                                    }
                                    .lerp_to_gamma(egui::Color32::WHITE, 0.3);

                                    let button = egui::Button::new(button_text).fill(button_colour);

                                    if ui.add(button).clicked() {
                                        stem.stem.ty = match stem.stem.ty {
                                            StemType::Note => StemType::BGM,
                                            StemType::BGM => StemType::Note
                                        }
                                    }
                                    }

                                    let lock_text = if stem.locked {
                                        "🔒"
                                    } else {
                                        "🔓"
                                    };

                                    if ui.button(lock_text).on_hover_text("Prevent slices from being altered on this stem (Key: L)").clicked() {
                                        stem.locked = !stem.locked;
                                    }

                                    {
                                        let mut remove_from_group = false;

                                        if let Some(group) = &mut stem.stem.group { 
                                            ui.horizontal(|ui|{ 
                                                let text_edit = egui::TextEdit::singleline(group).desired_width(100.0);
                                                ui.add(text_edit);

                                                if ui.button("X").clicked() {
                                                    remove_from_group = true;
                                                }
                                            });
                                        }
                                        else {
                                            if ui.button("Set Group").clicked() {
                                                stem.stem.group = Some("Group X".into()); 
                                            }

                                            if ui.add_enabled(!stem.locked, Button::new("Delete Stem ⚠️")).clicked() {
                                                stem_to_delete = Some(stem_i);
                                            }
                                        } 

                                        if remove_from_group {
                                            stem.stem.group = None;
                                        }
                                    }
                                },
                            );


                            let group_colour_opacity = self.group_colour_opacity.clamp(0.0, 1.0);

                            const BASE_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(35);
                            let background_colour = if let Some(group) = &stem.stem.group {
                                let colour = {
                                    let mut hasher = std::hash::DefaultHasher::new();
                                    group.hash(&mut hasher);
                                    let hash = hasher.finish();

                                    let hue = (hash & 0xffff) as f32 / 65535.0;

                                    let saturation_ratio = ((hash >> 16) & 0xff) as f32 / 255.0;
                                    let value_ratio = ((hash >> 24) & 0xff) as f32 / 255.0;

                                    egui::ecolor::Hsva::new(
                                        hue, 
                                        0.5 + saturation_ratio * 0.3,
                                        0.6 + value_ratio * 0.25,
                                        1.0
                                        ).into()

                                    /* old code for rgb from hash instead
                                    let colour: u64 = hash % (256 * 256 * 256);

                                    let r = (colour & 0xff) as u8;
                                    let b = ((colour >> 16) & 0xff) as u8;

                                    // egui::Color32::from_rgba_premultiplied(r, g, b, 255)
                                    */

                                };

                                BASE_BACKGROUND_COLOR.lerp_to_gamma(colour, group_colour_opacity)
                            } else {
                                BASE_BACKGROUND_COLOR
                            };


                            let (rect, event) = draw_stem(
                                ui,
                                background_colour,
                                stem,
                                &self.project.timing,
                                &self.input_state,
                                self.display_start,
                                end_time_point,
                            )
                            .unwrap();

                            if let Some(col) = &stem.stem.group{
                            }

                            if stem.locked{ 
                                ui.painter().rect_filled(rect, 0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 125));
                            }

                            if let Some(event) = event && 
                                // we need hover to make locking/re-locking work
                                !(stem.locked && !matches!(event, StemEvent::Hovering)) { 
                                    match event {
                                        StemEvent::Hovering => {
                                            for slice in std::mem::take(&mut self.midi_file_slices) {
                                                stem.stem.slices.insert(slice);
                                            }
                                            if self.input_state.l_pressed {
                                                stem.locked = !stem.locked;
                                            }
                                        }
                                        StemEvent::PlayAudio(sample_clicked)
                                            if let Some(audio) = &stem.audio =>
                                        {
                                            // If there is a slice before our cursor
                                            if let Some((first_slice_index, first_slice)) =
                                                stem.stem.slices.iter().enumerate().rfind(|(_, slice)| {
                                                    let Ok(v) = calculate_num_samples(
                                                        Default::default(),
                                                        slice.time_point,
                                                        self.project.sample_rate,
                                                        1,
                                                        &self.project.timing,
                                                    ) else {
                                                        return false;
                                                    };

                                                    v < sample_clicked
                                                })
                                                && let Some(second_slice) =
                                                    stem.stem.slices.0.get(first_slice_index + 1)
                                            {
                                                let start = first_slice.time_point;
                                                let end = second_slice.time_point;

                                                let start_sample_index = start
                                                    .samples_from_start(
                                                        audio.sample_rate(),
                                                        &self.project.timing,
                                                    )
                                                    .unwrap_or(0);
                                                let end_sample_index = end
                                                    .samples_from_start(
                                                        audio.sample_rate(),
                                                        &self.project.timing,
                                                    )
                                                    .unwrap_or(0);

                                                // TODO: Put make this pre-trim it before fetching the channels?
                                                if let Ok(channels) = stem
                                                    .audio
                                                    .as_ref()
                                                    .expect("NO AUDIO IN STEM?")
                                                    .channels()
                                                    .into_iter()
                                                    .map(|channel| {
                                                        channel
                                                            .get(start_sample_index..end_sample_index)
                                                            .map(|v| v.to_vec())
                                                            .ok_or("bad channel")
                                                    })
                                                    .collect::<Result<Vec<_>, _>>()
                                                {
                                                    let playback = crate::audio_player::AudioPlayback::new(
                                                        // TODO: Make this not clone the channels completely
                                                        channels, None,
                                                    )
                                                    .expect("failed to add audio");

                                                    if let Some(audio_player) = &self.audio_player {
                                                        audio_player.add_audio(playback);
                                                    }
                                                } else {
                                                    log::error!("bad channel");
                                                }
                                            }
                                        }
                                        StemEvent::LeftClick(sample_clicked) => {
                                            log::trace!("clicked at {sample_clicked} samples");

                                            let Ok(time_point) = crate::audio::TimePoint::from_sample(
                                                sample_clicked,
                                                self.project.sample_rate.0,
                                                &self.project.timing,
                                            ) else {
                                                log::error!(
                                                    "failed to get time point from sample {sample_clicked}"
                                                );
                                                return;
                                            };

                                            stem.stem.slices_mut().insert(crate::project::Slice {
                                                time_point: time_point.quantised(self.slice_snapping),
                                            });
                                        }

                                        StemEvent::RightClick(sample_clicked) => {
                                            let Ok(time_point) = crate::audio::TimePoint::from_sample(
                                                sample_clicked,
                                                self.project.sample_rate.0,
                                                &self.project.timing,
                                            ) else {
                                                log::error!(
                                                    "failed to get time point from sample {sample_clicked}"
                                                );
                                                return;
                                            };

                                            const DELETE_DISTANCE: f64 = 0.075;

                                            stem.stem.slices.0.retain(|slice| {
                                                (slice.time_point - time_point).to_f64().abs()
                                                    > DELETE_DISTANCE * (self.zoom_level as f64)
                                            });
                                        }
                                        StemEvent::PlayAudio(_) => ()
                                    }
                            }
                        },
                    );
                    }

 
                    if let Some(stem_i) = stem_to_export {
                        let stem = self.project.stems.get(stem_i).expect("stem i not found");

                        if let Some(group) = &stem.stem.group {
                            // if in a group, find all stems in that group and export them together
                            let stems = self.project.stems.iter()
                                .filter(|stem| stem.stem.group.as_ref().is_some_and(|g| g == group)).collect::<Vec<_>>();

                            if !stems.is_empty() {
                                let stem_prefix = group;
                                let audio = stems.iter().filter_map(|stem|stem.audio.as_ref()).collect::<Vec<_>>();
                                let slices = stems.iter().fold(crate::project::Slices::default(), |mut acc, x|{
                                    acc.union(&x.stem.slices);
                                    acc
                                });

                                if let Err(e) = audio::export_stem(&export_dir, stem_prefix, &audio, &slices, &self.project.timing) {
                                    log::error!("bad export: {e}");
                                }
                            }
                        } else {
                            // if not in a group, just get the details from the one stem
                            
                            // TODO: Log "bad stem" if audio missing "bad stem"
                            // TODO: Log "bad audio" if audio path missing
                            
                            let Some(audio) = stem.audio.as_ref() else {panic!("")};
                            let slices = &stem.stem.slices;
                            let audio = &[audio];
                            let Some(stem_prefix) = stem.stem.audio_path.file_stem().and_then(|v|v.to_str()) else {panic!("")};

                            if let Err(e) = audio::export_stem(&export_dir, stem_prefix, audio, &slices, &self.project.timing) {
                                log::error!("bad export: {e}");
                            }
                        }
                    }

                    if let Some(stem_i) = stem_to_delete {
                        if stem_i >= self.project.stems.len() {
                            log::error!("Failed to remove stem {stem_i}: out of range");
                        }

                        self.project.stems.remove(stem_i);
                    }
                });

            let rect = ui.available_rect_before_wrap();

            // Allocate it so its being used
            ui.allocate_rect(rect, egui::Sense::all());

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/main/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}

enum StemEvent {
    Hovering,
    LeftClick(usize),
    RightClick(usize),
    PlayAudio(usize),
}

fn draw_stem(
    ui: &mut egui::Ui,
    background_color: egui::Color32,
    live_stem: &LiveStem,
    timing: &audio::Timing,
    input_state: &InputState,
    start_time: crate::audio::TimePoint,
    end_time: crate::audio::TimePoint,
) -> Result<(egui::Rect, Option<StemEvent>), crate::Error> {
    let bpm_changes = timing.bpm_changes();

    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::new(ui.available_width(), STEM_HEIGHT),
        egui::Sense::click(),
    );

    const NUM_CHANNELS: u16 = 1;

    let sample_rate = live_stem
        .audio
        .as_ref()
        .map(|v| v.sample_rate().into())
        .unwrap_or_default();

    let start_sample = calculate_num_samples(
        crate::audio::TimePoint::default(),
        start_time,
        sample_rate,
        NUM_CHANNELS,
        timing,
    )?;

    let end_sample = calculate_num_samples(
        crate::audio::TimePoint::default(),
        end_time,
        sample_rate,
        NUM_CHANNELS,
        timing,
    )?;

    let visual_samples = end_sample - start_sample;

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, background_color);

    const SLICE_COLOUR: egui::Color32 = egui::Color32::WHITE;
    let stroke = egui::Stroke::new(3.0f32, SLICE_COLOUR);

    let mouse_x_ratio = {
        let x1 = rect.min.x;
        let x2 = rect.max.x;

        ((input_state.mouse_pos.unwrap_or_default().x - x1) / (x2 - x1)).clamp(0.0, 1.0)
    };

    if let Some(audio) = &live_stem.audio {
        let num_samples = end_sample - start_sample;
        let starting_sample = start_time.samples_from_start(audio.sample_rate(), timing)?;

        // TODO: Move this somewhere else?
        let visual_density = 6000;

        let waveform_stroke =
            egui::Stroke::new(1.5f32, egui::Color32::from_gray(190).linear_multiply(0.7));

        audio.draw_channel(
            0,
            Some(visual_density),
            starting_sample,
            num_samples,
            &rect,
            &painter,
            waveform_stroke,
        );
    }

    let slices = live_stem
        .stem
        .slices
        .iter()
        .enumerate()
        .filter_map(|(slice_i, slice)| {
            if !(start_time..=end_time).contains(&slice.time_point) {
                return None;
            }

            Some((slice_i, slice.clone()))
        });

    let measure_stroke = egui::Stroke::new(2.0f32, egui::Color32::DARK_BLUE.linear_multiply(0.7));
    for i in start_time.to_integer()..end_time.to_integer() {
        let measure_sample_index = calculate_num_samples(
            Default::default(),
            crate::audio::TimePoint::from_measure(i),
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if measure_sample_index < start_sample || end_sample < measure_sample_index {
            continue;
        }

        let ratio = (measure_sample_index - start_sample) as f64 / (visual_samples) as f64;

        let tx = rect.min.x + (ratio as f32) * (rect.max.x - rect.min.x);

        let points = [
            egui::Pos2 {
                x: tx,
                y: rect.min.y,
            },
            egui::Pos2 {
                x: tx,
                y: rect.max.y,
            },
        ];

        painter.line_segment(points, measure_stroke);
    }

    let bpm_change_stroke = egui::Stroke::new(6.0f32, egui::Color32::RED.linear_multiply(0.7));
    for bpm_change in bpm_changes {
        let sample = calculate_num_samples(
            Default::default(),
            bpm_change.time_point,
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if sample < start_sample || end_sample < sample {
            continue;
        }

        let ratio = (sample - start_sample) as f64 / (visual_samples) as f64;
        let tx = rect.min.x + (ratio as f32) * (rect.max.x - rect.min.x);

        let points = [
            egui::Pos2 {
                x: tx,
                y: rect.min.y,
            },
            egui::Pos2 {
                x: tx,
                y: rect.max.y,
            },
        ];

        painter.line_segment(points, bpm_change_stroke);
    }

    for (i, slice) in slices {
        let sample = calculate_num_samples(
            Default::default(),
            slice.time_point,
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if sample < start_sample || end_sample < sample {
            break;
        }

        let ratio = (sample - start_sample) as f64 / (visual_samples) as f64;

        let tx = rect.min.x + (ratio as f32) * (rect.max.x - rect.min.x);

        let points = [
            egui::Pos2 {
                x: tx,
                y: rect.min.y,
            },
            egui::Pos2 {
                x: tx,
                y: rect.max.y,
            },
        ];

        painter.line_segment(points, stroke);
        painter.text(
            [tx, rect.max.y].into(),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{:0>2}",
                base62::encode(i as u64 + live_stem.stem.starting_keysound.unwrap_or(1))
            ),
            egui::FontId::default(),
            SLICE_COLOUR,
        );
    }

    let mut event = None;

    if let Some(mouse_pos) = &input_state.mouse_pos
        && rect.contains(*mouse_pos)
    {
        // default to hovering if no other event is met
        event = Some(StemEvent::Hovering);

        let sample_clicked =
            (start_sample as f64 + mouse_x_ratio as f64 * visual_samples as f64).round() as usize;

        if response.middle_clicked() || input_state.g_pressed {
            let sample_clicked = (start_sample as f64
                + mouse_x_ratio as f64 * visual_samples as f64)
                .round() as usize;

            event = Some(StemEvent::PlayAudio(sample_clicked));
        }

        if input_state.lmb_down {
            event = Some(StemEvent::LeftClick(sample_clicked));
        } else if input_state.rmb_down {
            event = Some(StemEvent::RightClick(sample_clicked));
        }
    }

    Ok((rect, event))
}

fn export_bms_file(
    path: impl AsRef<std::path::Path>,
    project: &crate::project::Project,
) -> Result<(), crate::Error> {
    let bms = bms_rs::bms::model::Bms::try_from(project.clone())?;

    let s = bms
        .unparse::<bms_rs::bms::command::channel::mapper::KeyLayoutBeat>()
        .into_iter()
        .map(|token| token.to_string())
        .collect::<Vec<String>>()
        .join("\n");

    std::fs::write(path, s)?;

    Ok(())
}
