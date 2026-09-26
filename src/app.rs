use crate::audio::{self, calculate_num_samples};
use audio::RatioExt as _;
use egui::{Button, emath::Numeric as _};
use std::hash::{Hash as _, Hasher as _};

pub const STEM_HEIGHT: f32 = 200.0;

#[derive(Default)]
struct InputState {
    pub mouse_pos: Option<egui::Pos2>,
    pub lmb_down: bool,
    pub rmb_down: bool,
    pub command_down: bool,
    pub scroll_delta: egui::Vec2,
    pub space_pressed: bool,
    pub shift_pressed: bool,
    pub zoom_delta: egui::Vec2,
    pub home_pressed: bool,
    pub g_pressed: bool,
    pub l_pressed: bool,
    pub f5_pressed: bool,
    pub esc_pressed: bool,
    pub number_pressed: Option<u8>,
    pub save_pressed: bool,
    pub ref_paste_pressed: bool,
    pub paste_pressed: bool,
}

impl InputState {
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        ctx.input_mut(|i| {
            let mut paste_pressed = false;
            let mut ref_paste_pressed = false;

            let shift_pressed = i.modifiers.shift;

            let command_down = i.modifiers.cmd_ctrl_matches(egui::Modifiers::COMMAND);

            for event in &i.events {
                #[expect(clippy::single_match)]
                match event {
                    egui::Event::Paste(_) => {
                        if shift_pressed {
                            ref_paste_pressed = true;
                        } else {
                            paste_pressed = true;
                        }
                    }
                    _ => (),
                }
            }

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
                space_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                shift_pressed: i.modifiers.shift,
                zoom_delta: i.zoom_delta_2d(),
                home_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::Home),
                g_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::G),
                l_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::L),
                f5_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::F5),
                esc_pressed: i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                number_pressed: first_number_pressed,
                save_pressed: i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                )),
                paste_pressed,
                ref_paste_pressed,
                command_down,
            }
        })
    }
}

#[derive(Default, Debug)]
enum ProjectStatus {
    #[default]
    None,
    Loaded,
    Failed,
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

    lock_all_on_startup: bool,

    #[serde(skip)]
    quit_application: bool,

    #[serde(skip)]
    copy_from: Option<usize>,
    #[serde(skip)]
    copy_to: Option<usize>,

    #[serde(skip)]
    selection: Selection,

    #[serde(skip)]
    painting_keysound: Option<(usize, audio::TimePoint)>,

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

    #[serde(skip)]
    stem_events: Vec<(usize, StemEvent)>,
}

#[derive(Debug)]
struct LiveStem {
    stem: crate::project::Stem,
    audio: Option<crate::audio::AudioFile>,
    locked: bool,
    best_channel: u16,
}

impl From<crate::project::Stem> for LiveStem {
    fn from(stem: crate::project::Stem) -> Self {
        Self {
            stem,
            audio: None,
            locked: false,
            best_channel: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct LiveProject {
    sample_rate: crate::project::SampleRate,
    stems: Vec<LiveStem>,
    timing: audio::Timing,
    samples_fadeout: u64,
}

impl std::convert::TryFrom<crate::project::Project> for LiveProject {
    type Error = crate::Error;

    fn try_from(project: crate::project::Project) -> Result<Self, Self::Error> {
        let crate::project::Project {
            sample_rate,
            stems,
            timing,
            samples_fadeout,
        } = project;

        Ok(Self {
            sample_rate,
            stems: stems.into_iter().map(|v| v.into()).collect(),
            timing,
            samples_fadeout,
        })
    }
}

impl LiveProject {
    fn as_project(&self) -> crate::project::Project {
        let Self {
            sample_rate,
            stems,
            timing,
            samples_fadeout,
        } = self;

        crate::project::Project {
            sample_rate: *sample_rate,
            stems: stems.iter().map(|stem| stem.stem.clone()).collect(),
            timing: timing.clone(),
            samples_fadeout: *samples_fadeout,
        }
    }

    fn groups(
        &self,
    ) -> (
        std::collections::HashMap<&str, Vec<&LiveStem>>,
        Vec<&LiveStem>,
    ) {
        let mut groups = std::collections::HashMap::new();
        let mut strays = vec![];

        for stem in &self.stems {
            if let Some(group) = &stem.stem.group {
                groups
                    .entry(group.as_str())
                    .or_insert_with(Vec::new)
                    .push(stem);
            } else {
                strays.push(stem);
            }
        }

        (groups, strays)
    }

    fn refresh(&mut self) {
        self.stems.sort_by(|stem1, stem2| {
            let g1 = &stem1.stem.group;
            let g2 = &stem2.stem.group;

            if g1.is_some() && g2.is_none() {
                std::cmp::Ordering::Less
            } else if g1.is_none() && g2.is_some() {
                std::cmp::Ordering::Greater
            } else {
                g1.cmp(g2).reverse()
            }
        });

        for stem in &mut self.stems {
            stem.best_channel = stem
                .audio
                .as_ref()
                .map(|audio| audio.best_channel_index(None))
                .unwrap_or(0);
        }
    }
}

#[derive(Default, Debug)]
struct Selection {
    from: Option<(usize, audio::TimePoint)>,
    to: Option<audio::TimePoint>,
}

impl Selection {
    pub fn select(&mut self, index: usize, point: audio::TimePoint) {
        match (self.from, self.to) {
            (None, _) => {
                if self.from.is_none() {
                    self.from = Some((index, point));
                    self.to = None;
                }
                log::info!("began selection from {index}:{point}");
            }
            (Some((i, from)), None) => {
                self.to = Some(point);
                log::info!("selecting from {i}:{from} to {index}:{point}");
            }
            (Some(_), Some(_)) => {
                self.from = Some((index, point));
                self.to = None;
                log::info!("began selection from {index}:{point}");
            }
        }
    }

    pub fn get(&self) -> Option<(usize, std::ops::Range<audio::TimePoint>)> {
        if let Some((index, from)) = &self.from
            && let Some(to) = &self.to
        {
            Some((*index, *from..*to))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.from = None;
        self.to = None;
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
            copy_from: None,
            copy_to: None,
            selection: Selection::default(),
            stem_events: vec![],
            painting_keysound: None,
            lock_all_on_startup: true,
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

    fn save_to_disk(&self) -> Result<(), crate::Error> {
        crate::project::save_project(
            &self.project.as_project(),
            self.project_path
                .clone()
                .map(crate::project::normalise_project_path)
                .unwrap_or_else(|| std::path::PathBuf::from("./project.jonnah")),
        )
    }

    fn default_export_dir(&self) -> std::path::PathBuf {
        self.project_path
            .as_ref()
            .map(|v| v.join("out"))
            .unwrap_or_else(|| "./".into())
    }

    fn draw_top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_panel").show_inside(ui, |ui|{
            ui.horizontal_top(|ui|{
                ui.menu_button("File", |ui| {
                    if ui.button("Save").clicked()
                        && let Err(e) = self.save_to_disk() {
                            log::error!("failed to save project: {e}");
                        }

                    if ui.button("Quit").clicked() {
                        self.quit_application = true;
                    }
                });

                ui.menu_button("Analyse", |ui| {
                    ui.menu_button("Calculate Initial Starting Keysounds", |ui|{
                        let wiggles = [0u64, 5, 10, 50];

                        for wiggle in wiggles {
                            if ui.button(format!("Wiggle room: {wiggle}")) .on_hover_text("Calculate the starting keysound for each stem, leaving small gaps between each one for wiggle room.\n\nThis will permanently alter your project, and should only be used ONCE to get initial keysound values for your stems.")
                                .clicked() {
                                let mut_stems = self.project.stems.iter_mut().map(|live| &mut live.stem).collect::<Vec<_>>();
                                if let Err(e) = crate::project::calculate_initial_keysounds(mut_stems, wiggle) {
                                    log::error!("failed to calculate initial keysounds: {e}");
                                }
                                else {
                                    log::info!("Successfully calculated initial keysounds. Remember to save your project!");
                                }
                            }
                        }
                    })
                });

                ui.menu_button("Settings", |ui| {
                    if ui.selectable_label(self.lock_all_on_startup, "Lock Stems On Startup").clicked() {
                        self.lock_all_on_startup = !self.lock_all_on_startup;
                    }
                });

                ui.add_space(16.0);
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
    }

    fn draw_options(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("JonnahSlicer v{}", env!("CARGO_PKG_VERSION")));

                #[expect(clippy::unwrap_used, reason = "if this is poisoned we are cooked")]
                let mut diagnostic = self.previous_diagnostic.lock().unwrap();

                if diagnostic.as_ref().is_some_and(|(time, _)| {
                    std::time::SystemTime::now()
                        .duration_since(*time)
                        .unwrap_or(std::time::Duration::from_secs(10))
                        >= std::time::Duration::from_secs(10)
                }) {
                    *diagnostic = None;
                }

                diagnostic.as_ref().inspect(|(_, msg)| {
                    ui.label(msg);
                });
            });

            ui.horizontal(|ui| {
                if ui
                    .button("⟳")
                    .on_hover_text("Refresh the project, ordering stems by group (Key: F5)")
                    .clicked()
                    || self.input_state.f5_pressed
                {
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

                ui.add(
                    egui::Slider::new(&mut self.group_colour_opacity, 0.0..=1.0)
                        .max_decimals(2)
                        .text("Group Opacity"),
                );

                ui.separator();

                if let Some(audio_player) = &mut self.audio_player {
                    if ui
                        .add(egui::Slider::new(&mut self.audio_volume, 0.0..=1.0).text("Volume"))
                        .changed()
                    {
                        audio_player.set_volume(self.audio_volume);
                    }
                } else {
                    ui.colored_label(egui::Color32::RED, "audio player not initialised");
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.project.samples_fadeout)
                            .range(0..=20_000)
                            .custom_formatter(|v, _| {
                                let v = v as u64;
                                format!("{v} samples")
                            }),
                    );

                    let fadeout_ms = self.project.samples_fadeout.saturating_mul(1000) as f64
                        / self.project.sample_rate.0 as f64;
                    ui.label("Fadeout Length")
                        .on_hover_text(format!("{fadeout_ms:.5}ms"));
                });

                ui.separator();

                if ui.button("Export All Stems").clicked() {
                    let mut exported = None;

                    let (groups, strays) = self.project.groups();

                    let sorted = {
                        let mut group_keys = groups.keys().clone().collect::<Vec<_>>();
                        group_keys.sort();
                        group_keys.into_iter().map(|key| (key, &groups[*key]))
                    };

                    for (group_name, group) in sorted {
                        let wrap_export_stem = || {
                            let slices = group.iter().fold(
                                crate::project::Slices::default(),
                                |mut acc, live_stem| {
                                    acc.union(&live_stem.stem.slices);
                                    acc
                                },
                            );

                            if slices.0.is_empty() {
                                return Ok(());
                            }

                            let audios = group
                                .iter()
                                .map(|group| group.audio.as_ref().ok_or("no audio file available"))
                                .collect::<Result<Vec<_>, _>>()?;

                            audio::export_stem(
                                self.default_export_dir(),
                                group_name,
                                &audios,
                                &slices,
                                &self.project.timing,
                                self.project.samples_fadeout,
                            )
                        };

                        if let Err(e) = wrap_export_stem() {
                            log::error!("failed to export all stems: {e}");
                            exported = None;
                            break;
                        }

                        *exported.get_or_insert(0usize) += 1;
                    }

                    for stray in strays {
                        if stray.stem.slices.0.is_empty() {
                            continue;
                        }

                        let wrap_export_stem = || {
                            let audio = stray.audio.as_ref().ok_or("no stem")?;
                            let Some(export_prefix) = stray.stem.export_prefix() else {
                                return Err("bad stem prefix")?;
                            };

                            let slices = &stray.stem.slices;

                            audio::export_stem(
                                self.default_export_dir(),
                                export_prefix,
                                &[audio],
                                slices,
                                &self.project.timing,
                                self.project.samples_fadeout,
                            )
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
                    let export_filepath = self.default_export_dir().join("out.bms");

                    if let Err(e) = export_bms_file(&export_filepath, &self.project.as_project()) {
                        log::error!("failed to export BMS file: {e}");
                    } else {
                        log::info!("Exported bms file at {}", export_filepath.display());
                    }
                }
            });

            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.label("Snapping: ");
                const SNAPPINGS: [u16; 14] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128];

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
        });
    }

    fn draw_main_ui(&mut self, ui: &mut egui::Ui) {
        let display_length =
            num_rational::Ratio::new(8 * (256.0 * self.zoom_level).round() as i64, 256);

        let end_time_point = self.display_start + display_length.trunc();

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

        let mut stem_to_export = None;
        let mut stem_to_delete = None;

        for (stem_i, stem) in self.project.stems.iter_mut().enumerate() {
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
                                    stem.stem.starting_keysound = match stem.stem.starting_keysound
                                    {
                                        Some(_) => None,
                                        None => Some(1),
                                    };
                                }
                                if let Some(starting_keysound) = &mut stem.stem.starting_keysound {
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
                                    StemType::BGM => "BGM",
                                };

                                let button_colour = match stem.stem.ty {
                                    StemType::Note => egui::Color32::BLUE,
                                    StemType::BGM => egui::Color32::RED,
                                }
                                .lerp_to_gamma(egui::Color32::WHITE, 0.3);

                                let button = egui::Button::new(button_text).fill(button_colour);

                                if ui.add(button).clicked() {
                                    stem.stem.ty = match stem.stem.ty {
                                        StemType::Note => StemType::BGM,
                                        StemType::BGM => StemType::Note,
                                    }
                                }
                            }

                            let lock_text = if stem.locked { "🔒" } else { "🔓" };

                            if ui
                                .button(lock_text)
                                .on_hover_text(
                                    "Prevent slices from being altered on this stem (Key: L)",
                                )
                                .clicked()
                            {
                                stem.locked = !stem.locked;
                            }

                            {
                                let mut remove_from_group = false;

                                if let Some(group) = &mut stem.stem.group {
                                    ui.horizontal(|ui| {
                                        let text_edit =
                                            egui::TextEdit::singleline(group).desired_width(100.0);
                                        ui.add(text_edit);

                                        if ui.button("X").clicked() {
                                            remove_from_group = true;
                                        }
                                    });
                                } else {
                                    if ui.button("Set Group").clicked() {
                                        stem.stem.group = Some("Group X".into());
                                    }

                                    if ui
                                        .add_enabled(!stem.locked, Button::new("Delete Stem ⚠️"))
                                        .clicked()
                                    {
                                        stem_to_delete = Some(stem_i);
                                    }
                                }

                                if remove_from_group {
                                    stem.stem.group = None;
                                }
                            }

                            ui.horizontal(|ui| {
                                let copy_text = if self.copy_from == Some(stem_i) {
                                    "Cancel Copy"
                                } else if self.copy_from.is_some() {
                                    "Paste"
                                } else {
                                    "Copy"
                                };

                                if ui.button(copy_text).clicked() {
                                    if self.copy_from == Some(stem_i) {
                                        self.copy_from = None;
                                    } else if self.copy_from.is_some() {
                                        self.copy_to = Some(stem_i);
                                    } else {
                                        self.copy_from = Some(stem_i);
                                    }
                                }
                            });
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
                                1.0,
                            )
                            .into()
                        };

                        BASE_BACKGROUND_COLOR.lerp_to_gamma(colour, group_colour_opacity)
                    } else {
                        BASE_BACKGROUND_COLOR
                    };

                    let (_rect, event) = draw_stem(
                        ui,
                        background_colour,
                        stem,
                        &self.project.timing,
                        &self.input_state,
                        self.display_start,
                        end_time_point,
                    )
                    .expect("failed to draw stem");

                    // TODO: implement drawing the selection box over the stem
                    /*
                    if let Some((i, range)) = self.selection.get()
                        && i == stem_i
                    {
                        let start = calculate_num_samples(
                            crate::audio::TimePoint::default(),
                            self.display_start,
                            self.project.sample_rate,
                            1,
                            &self.project.timing,
                        )
                        .unwrap();

                        let end = calculate_num_samples(
                            crate::audio::TimePoint::default(),
                            end_time_point,
                            self.project.sample_rate,
                            1,
                            &self.project.timing,
                        )
                        .unwrap();
                    }
                    */

                    if let Some(event) = event
                        && !(stem.locked && !matches!(event, StemEvent::Hovering(_)))
                    {
                        self.stem_events.push((stem_i, event));
                    }
                },
            );
        }

        if let Some(stem_i) = stem_to_export {
            let export_dir = self.default_export_dir();
            if let Err(e) = self.export_stem(export_dir, stem_i) {
                log::error!("failed to export stem: {e}");
            }
        }

        if let Some(stem_i) = stem_to_delete {
            if stem_i >= self.project.stems.len() {
                log::error!("Failed to remove stem {stem_i}: out of range");
            }

            self.project.stems.remove(stem_i);
        }
    }

    fn export_stem(
        &self,
        export_dir: impl AsRef<std::path::Path>,
        stem_i: usize,
    ) -> Result<(), crate::Error> {
        let stem = self
            .project
            .stems
            .get(stem_i)
            .ok_or_else(|| format!("stem[{stem_i}] not found"))?;

        if let Some(group) = &stem.stem.group {
            // if in a group, find all stems in that group and export them together
            let stems = self
                .project
                .stems
                .iter()
                .filter(|stem| stem.stem.group.as_ref().is_some_and(|g| g == group))
                .collect::<Vec<_>>();

            if stems.is_empty() {
                return Ok(());
            }

            let stem_prefix = group;
            let audio = stems
                .iter()
                .filter_map(|stem| stem.audio.as_ref())
                .collect::<Vec<_>>();
            let slices = stems
                .iter()
                .fold(crate::project::Slices::default(), |mut acc, x| {
                    acc.union(&x.stem.slices);
                    acc
                });

            return audio::export_stem(
                &export_dir,
                stem_prefix,
                &audio,
                &slices,
                &self.project.timing,
                self.project.samples_fadeout,
            );
        }

        // if not in a group, just get the details from the one stem

        // TODO: Log "bad stem" if audio missing "bad stem"
        // TODO: Log "bad audio" if audio path missing

        let Some(audio) = stem.audio.as_ref() else {
            return Err("audio missing".into());
        };
        let slices = &stem.stem.slices;
        let audio = &[audio];
        let Some(stem_prefix) = stem.stem.audio_path.file_stem().and_then(|v| v.to_str()) else {
            panic!("")
        };

        audio::export_stem(
            export_dir,
            stem_prefix,
            audio,
            slices,
            &self.project.timing,
            self.project.samples_fadeout,
        )
    }

    fn handle_stem_events(&mut self) {
        const SENSE_DISTANCE: f64 = 0.075;

        for (stem_i, event) in std::mem::take(&mut self.stem_events) {
            let Some(stem) = self.project.stems.get_mut(stem_i) else {
                continue;
            };

            match event {
                StemEvent::Hovering(sample_clicked) => {
                    for slice in std::mem::take(&mut self.midi_file_slices) {
                        stem.stem.slices.insert(slice);
                    }
                    if self.input_state.l_pressed {
                        stem.locked = !stem.locked;
                    }

                    let Ok(click_point) = crate::audio::TimePoint::from_sample(
                        sample_clicked,
                        self.project.sample_rate.0,
                        &self.project.timing,
                    ) else {
                        log::error!("failed to get time point from sample {sample_clicked}");
                        continue;
                    };

                    let quantised = click_point.quantised(self.slice_snapping);

                    if self.input_state.paste_pressed | self.input_state.ref_paste_pressed {
                        let mut handle_paste = || -> Result<_, crate::Error> {
                            use crate::project::SliceKeysound;

                            let Some((index, range)) = self.selection.get() else {
                                return Ok(None);
                            };

                            let (from, to) = (range.start, range.end);

                            let mut translated = self
                                .project
                                .stems
                                .get(index)
                                .map(|v| v.stem.slices.clone())
                                .ok_or("can't copy from no stem")?;

                            translated
                                .0
                                .retain(|v| from <= v.time_point && v.time_point <= to);

                            for slice in &mut translated.0 {
                                if self.input_state.ref_paste_pressed
                                    && matches!(slice.keysound_id, SliceKeysound::Auto)
                                {
                                    // can use own time point because it hasn't been translated yet
                                    slice.keysound_id = SliceKeysound::Reference(slice.time_point);
                                }

                                slice.time_point += quantised - from;
                            }

                            self.project.stems[stem_i].stem.slices.union(&translated);

                            Ok(Some(translated.0.len()))
                        };

                        match handle_paste() {
                            Ok(Some(num_copied)) => {
                                log::info!(
                                    "Stem {stem_i}: pasted {num_copied} samples at {quantised}"
                                );
                            }
                            Ok(None) => {
                                log::info!("No selection available");
                            }
                            Err(e) => {
                                log::error!("failed to paste: {e}");
                            }
                        }
                    } else if self.input_state.space_pressed {
                        self.selection.select(stem_i, quantised);
                    }
                }
                StemEvent::PlayAudio(sample_clicked) if let Some(audio) = &stem.audio => {
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
                        && let Some(second_slice) = stem.stem.slices.0.get(first_slice_index + 1)
                    {
                        let start = first_slice.time_point;
                        let end = second_slice.time_point;

                        let start_sample_index = start
                            .samples_from_start(audio.sample_rate(), &self.project.timing)
                            .unwrap_or(0);
                        let end_sample_index = end
                            .samples_from_start(audio.sample_rate(), &self.project.timing)
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

                            if let Some(audio_player) = &self.audio_player
                                && let Err(e) = audio_player.add_audio(playback)
                            {
                                log::error!("failed to add audio: {e}");
                            }
                        } else {
                            log::error!("bad channel");
                        }
                    }
                }
                StemEvent::LeftClick(sample_clicked) => {
                    let Ok(click_point) = crate::audio::TimePoint::from_sample(
                        sample_clicked,
                        self.project.sample_rate.0,
                        &self.project.timing,
                    ) else {
                        log::error!("failed to get time point from sample {sample_clicked}");
                        continue;
                    };

                    // Command + LMB = paint keysound
                    if self.input_state.command_down {
                        let Some((from_slice_i, from_tp)) = &self.painting_keysound else {
                            log::info!(
                                "nothing to paint with! use Cmd/Ctrl + RMB to begin painting"
                            );
                            continue;
                        };

                        let Some(paint_from) =
                            self.project.stems.get(*from_slice_i).and_then(|live| {
                                live.stem.slices.query_dereferenced(*from_tp).cloned()
                            })
                        else {
                            log::warn!("stem {from_slice_i}: slice at tp {from_tp} does not exist");
                            continue;
                        };

                        let Some((paint_to, _)) =
                            &mut self.project.stems.get_mut(stem_i).and_then(|live| {
                                live.stem
                                    .slices
                                    .closest_bound_mut(click_point, SENSE_DISTANCE)
                            })
                        else {
                            log::warn!("no slice close enough");
                            continue;
                        };

                        if paint_to.time_point < *from_tp {
                            log::warn!("a reference must be placed AFTER the original keysound");
                            continue;
                        }

                        paint_to.keysound_id =
                            crate::project::SliceKeysound::Reference(paint_from.time_point);

                        log::info!("stem {stem_i}: painted a slice at {}", paint_to.time_point);
                    }
                    // LMB = create slice
                    else {
                        // shift + left click = initiate copy
                        if !self.input_state.shift_pressed {
                            log::trace!("clicked at {sample_clicked} samples");

                            let time_point = click_point.quantised(self.slice_snapping);
                            stem.stem
                                .slices_mut()
                                .insert(crate::project::Slice::new(time_point));
                        }
                    }
                }

                StemEvent::RightClick(sample_clicked) => {
                    let Ok(time_point) = crate::audio::TimePoint::from_sample(
                        sample_clicked,
                        self.project.sample_rate.0,
                        &self.project.timing,
                    ) else {
                        log::error!("failed to get time point from sample {sample_clicked}");
                        continue;
                    };

                    // Command + RMB = begin slice paint
                    if self.input_state.command_down {
                        if let Some((closest, _)) =
                            stem.stem.slices.closest_bound(time_point, SENSE_DISTANCE)
                        {
                            log::info!(
                                "stem {stem_i}: selected slice at {time_point} for painting"
                            );
                            self.painting_keysound = Some((stem_i, closest.time_point));
                        } else {
                            log::info!("no slice in range to copy for painting");
                        }
                    }
                    // RMB = delete slice
                    else {
                        stem.stem.slices.0.retain(|slice| {
                            (slice.time_point - time_point).to_f64().abs()
                                > SENSE_DISTANCE * (self.zoom_level as f64)
                        });
                    }
                }
                StemEvent::PlayAudio(_) => (),
            }
        }
    }
}

impl eframe::App for JonnahSlicer<'_> {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_stem_events();

        if self.input_state.esc_pressed {
            self.copy_from = None;
            self.copy_to = None;
            self.selection.clear();
            self.painting_keysound = None;

            if let Some(player) = &mut self.audio_player {
                player.stop();
            }

            log::info!("cleared selection");
        }

        if let Some(copy_from) = self.copy_from
            && let Some(copy_to) = self.copy_to
        {
            if let Some(from_slices) = self
                .project
                .stems
                .get(copy_from)
                .map(|live| live.stem.slices.clone())
            {
                if let Some(to) = self.project.stems.get_mut(copy_to) {
                    to.stem.slices.union(&from_slices);
                }
            } else {
                log::error!("unable to copy from stem[{copy_from}]");
            }
        }

        if self.refresh_project {
            self.refresh_project = false;
            self.project.refresh();
        }

        self.input_state = InputState::from_ctx(ctx);
        if let Some(project_path) = &self.project_path {
            match &self.project_status {
                ProjectStatus::None => {
                    match crate::project::load_project(project_path)
                        .and_then(|project| project.try_into())
                    {
                        Ok(project) => {
                            self.project = project;

                            if self.lock_all_on_startup {
                                for stem in &mut self.project.stems {
                                    stem.locked = true;
                                }
                            }

                            if self.audio_player.is_some() {
                                let new_audio_player =
                                    crate::audio_player::AudioPlayer::new(self.project.sample_rate)
                                        .ok();
                                self.audio_player = new_audio_player
                                    .inspect(|player| player.set_volume(self.audio_volume));
                            }

                            self.project_status = ProjectStatus::Loaded;
                        }
                        Err(e) => {
                            log::error!("failed to load project: {e}");
                            self.project_status = ProjectStatus::Failed;
                        }
                    }
                }
                ProjectStatus::Failed | ProjectStatus::Loaded => (),
            }
        }
        if self.input_state.save_pressed
            && let Err(e) = self.save_to_disk()
        {
            log::error!("failed to save project: {e}");
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
            #[expect(deprecated, reason = "this doesn't work atm, no point refactoring")]
            egui::Panel::left("File Hover Preview").show(ctx, |_| {
                if !hovered.is_empty() {
                    for path in hovered.into_iter().filter_map(|file| file.path) {
                        #[expect(unused_variables, reason = "implement after fixing midi import")]
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
                        best_channel: 0,
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
                let audio = crate::audio::AudioFile::new(wav).expect("Bad wav");
                let best_channel = audio.best_channel_index(None);

                // TODO: FIX THIS
                st.audio = Some(audio);
                st.best_channel = best_channel;
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_top_bar(ui);
        self.draw_options(ui);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(jonnah) = &self.jonnah_image {
                jonnah.paint_at(ui, ui.content_rect());
            }

            ui.separator();

            egui::ScrollArea::vertical()
                .scroll_source(if self.input_state.shift_pressed {
                    egui::scroll_area::ScrollSource::NONE
                } else {
                    egui::scroll_area::ScrollSource::MOUSE_WHEEL
                })
                .show(ui, |ui| self.draw_main_ui(ui));

            let rect = ui.available_rect_before_wrap();

            // Allocate it so its being used
            ui.allocate_rect(rect, egui::Sense::all());

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/main/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

enum StemEvent {
    Hovering(usize),
    LeftClick(usize),
    RightClick(usize),
    PlayAudio(usize),
}

fn draw_line(painter: &egui::Painter, stroke: egui::Stroke, rect: egui::Rect, t: f32) {
    let tx = rect.min.x + t * (rect.max.x - rect.min.x);

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

    let start = calculate_num_samples(
        crate::audio::TimePoint::default(),
        start_time,
        sample_rate,
        NUM_CHANNELS,
        timing,
    )?;

    let end = calculate_num_samples(
        crate::audio::TimePoint::default(),
        end_time,
        sample_rate,
        NUM_CHANNELS,
        timing,
    )?;

    let width_samples = end - start;

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, background_color);

    let text = live_stem
        .stem
        .audio_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("error");

    const SLICE_COLOUR: egui::Color32 = egui::Color32::WHITE;
    let slice_stroke = egui::Stroke::new(3.0f32, SLICE_COLOUR);
    const REF_SLICE_COLOUR: egui::Color32 = egui::Color32::PURPLE;
    let ref_slice_stroke = egui::Stroke::new(3.0f32, REF_SLICE_COLOUR);

    let mouse_x_ratio = {
        let x1 = rect.min.x;
        let x2 = rect.max.x;

        ((input_state.mouse_pos.unwrap_or_default().x - x1) / (x2 - x1)).clamp(0.0, 1.0)
    };

    if let Some(audio) = &live_stem.audio {
        let num_samples = end - start;
        let starting_sample = start_time.samples_from_start(audio.sample_rate(), timing)?;

        // TODO: Move this somewhere else?
        let visual_density = 6000;

        let waveform_stroke =
            egui::Stroke::new(1.5f32, egui::Color32::from_gray(190).linear_multiply(0.7));

        audio.draw_channel(
            live_stem.best_channel,
            Some(visual_density),
            starting_sample,
            num_samples,
            &rect,
            &painter,
            waveform_stroke,
        );
    }

    for i in start_time.to_integer()..end_time.to_integer() {
        let measure_sample_index = calculate_num_samples(
            Default::default(),
            crate::audio::TimePoint::from_measure(i),
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if measure_sample_index < start || end < measure_sample_index {
            continue;
        }

        let ratio = (measure_sample_index - start) as f32 / (width_samples) as f32;

        let stroke = egui::Stroke::new(2.0f32, egui::Color32::DARK_BLUE.linear_multiply(0.7));
        draw_line(&painter, stroke, rect, ratio);
    }

    for bpm_change in bpm_changes {
        let sample = calculate_num_samples(
            Default::default(),
            bpm_change.time_point,
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if sample < start || end < sample {
            continue;
        }

        let ratio = (sample - start) as f32 / (width_samples) as f32;

        let stroke = egui::Stroke::new(6.0f32, egui::Color32::RED.linear_multiply(0.7));
        draw_line(&painter, stroke, rect, ratio);
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

    for (i, slice) in slices {
        let sample = calculate_num_samples(
            Default::default(),
            slice.time_point,
            sample_rate,
            NUM_CHANNELS,
            timing,
        )?;

        if sample < start || end < sample {
            break;
        }

        let ratio = (sample - start) as f32 / (width_samples) as f32;

        let obj_id = live_stem
            .stem
            .slices
            .keysound_index_of(i)
            .map(|index| index as u64 + live_stem.stem.starting_keysound.unwrap_or(1));

        let slice_label = if let Some(obj_id) = obj_id {
            format!("{:0>2}", base62::encode(obj_id))
        } else {
            "ERR".into()
        };

        let (stroke, colour) = if obj_id.is_some() {
            match slice.keysound_id {
                crate::project::SliceKeysound::Auto => (slice_stroke, SLICE_COLOUR),
                crate::project::SliceKeysound::Reference(_) => (ref_slice_stroke, REF_SLICE_COLOUR),
            }
        } else {
            (
                egui::Stroke::new(slice_stroke.width, egui::Color32::BLACK),
                egui::Color32::DARK_RED,
            )
        };

        draw_line(&painter, stroke, rect, ratio);
        let tx = rect.min.x + ratio * (rect.max.x - rect.min.x);
        painter.text(
            [tx, rect.max.y].into(),
            egui::Align2::LEFT_BOTTOM,
            slice_label,
            egui::FontId::default(),
            colour,
        );
    }

    const TEXT_PADDING: f32 = 4.0;
    painter.text(
        egui::Pos2::new(rect.min.x + TEXT_PADDING, rect.min.y + TEXT_PADDING),
        egui::Align2::LEFT_TOP,
        text,
        Default::default(),
        egui::Color32::from_gray(220),
    );

    if live_stem.locked {
        ui.painter().rect_filled(
            rect,
            0,
            egui::Color32::from_rgba_premultiplied(0, 0, 0, 125),
        );
    }

    let mut event = None;

    if let Some(mouse_pos) = &input_state.mouse_pos
        && rect.contains(*mouse_pos)
    {
        let sample_clicked =
            (start as f64 + mouse_x_ratio as f64 * width_samples as f64).round() as usize;

        // default to hovering if no other event is met
        event = Some(StemEvent::Hovering(sample_clicked));

        if response.middle_clicked() || input_state.g_pressed {
            let sample_clicked =
                (start as f64 + mouse_x_ratio as f64 * width_samples as f64).round() as usize;

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
