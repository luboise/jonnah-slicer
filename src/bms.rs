use crate::audio::RatioExt as _;

impl TryFrom<crate::audio::BpmChange> for bms_rs::bms::model::obj::BpmChangeObj {
    type Error = crate::Error;

    fn try_from(value: crate::audio::BpmChange) -> Result<Self, Self::Error> {
        let crate::audio::BpmChange { time_point, bpm } = value;

        Ok(Self {
            time: time_point.to_objtime()?,
            bpm: bpm.try_into()?,
        })
    }
}

pub const CHANNEL_MAP: [&[u8; 2]; 16] = [
    b"16", b"11", b"12", b"13", b"14", b"15", b"18", b"19", // left side
    b"21", b"22", b"23", b"24", b"25", b"28", b"29", b"26", // right side
];

pub const CHANNEL_NAMES: [&str; 16] = [
    "1-S", "1-1", "1-2", "1-3", "1-4", "1-5", "1-6", "1-7", // left side
    "2-1", "2-2", "2-3", "2-4", "2-5", "2-6", "2-7", "2-S", // right side
];

impl TryFrom<crate::project::Project> for bms_rs::bms::model::Bms {
    type Error = crate::Error;

    fn try_from(value: crate::project::Project) -> Result<Self, Self::Error> {
        let crate::project::Project {
            sample_rate: _,
            stems,
            timing: bpm_changes,
            samples_fadeout: _,
        } = value;

        let mut bpm_changes = bpm_changes.into_iter();

        let first_bpm = {
            let first_bpm_change = bpm_changes.next().ok_or("bpm_changes is empty")?;
            bms_rs::bms::model::StringValue::from_value(bms_rs::bmson::prelude::PositiveF64::new(
                first_bpm_change.bpm,
            )?)
        };

        let mut bpm = bms_rs::bms::model::bpm::BpmObjects {
            bpm: Some(first_bpm),
            ..Default::default()
        };

        for bpm_change in bpm_changes {
            bpm.push_bpm_change(
                bpm_change.try_into()?,
                &bms_rs::bms::parse::prompt::AlwaysUseNewer,
            )?;
        }

        let mut wav_files = std::collections::HashMap::new();
        let mut notes = bms_rs::bms::model::Notes::default();

        let mut groups = std::collections::HashSet::new();

        for stem in stems {
            if let Some(group) = &stem.group {
                if groups.contains(group) {
                    continue;
                } else {
                    groups.insert(group.clone());
                }
            }

            // base62 keysounds start at 01 not 00
            let obj_id = stem.starting_keysound.unwrap_or(1);

            let (wav_file_stem, file_extension) = if let Some(group) = &stem.group {
                (group.clone(), "wav".to_owned())
            } else {
                // TODO: Make this not fail?
                let (file_stem, file_extension) = (
                    stem.audio_path
                        .file_stem()
                        .and_then(|v| v.to_str())
                        .ok_or("no filename")?,
                    stem.audio_path
                        .extension()
                        .and_then(|v| v.to_str())
                        .ok_or("no extension")?,
                );

                (file_stem.to_owned(), file_extension.to_owned())
            };

            let mut obj_ids = vec![];

            for (slice_i, slice) in stem.slices.0.iter().enumerate() {
                if matches!(slice.keysound_id, crate::project::SliceKeysound::Auto) {
                    // otherwise, it's a new keysound. insert it
                    let mut encoded = base62::encode(obj_id + obj_ids.len() as u64);
                    if encoded.len() == 1 {
                        encoded.insert(0, '0');
                    }

                    let wav_path = format!("{wav_file_stem}_{:03}.{file_extension}", obj_ids.len());

                    let wav_obj_id = bms_rs::bms::command::ObjId::try_from(&encoded, true)?;
                    wav_files.insert(wav_obj_id, wav_path.into());

                    obj_ids.push(wav_obj_id);
                }

                let wav_obj_id = stem
                    .slices
                    .keysound_index_of(slice_i)
                    .and_then(|i| obj_ids.get(i))
                    .copied()
                    .ok_or_else(|| format!("slice {slice_i} points to non-existant keysound"))?;

                match stem.ty {
                    crate::project::StemType::Note(note_i) => {
                        let mapped = CHANNEL_MAP.get(note_i).unwrap_or(&CHANNEL_MAP[0]);

                        let note_channel_id = (**mapped)
                            .try_into()
                            .map_err(|e| format!("bad input: {e:#?}"))?;

                        notes.push_note(bms_rs::bms::model::obj::WavObj {
                            offset: slice.time_point.to_objtime()?,
                            channel_id: note_channel_id,
                            wav_id: wav_obj_id,
                        });
                    }
                    crate::project::StemType::BGM => {
                        notes.push_bgm::<bms_rs::bms::command::channel::mapper::KeyLayoutBeat>(
                            slice.time_point.to_objtime()?,
                            wav_obj_id,
                        );
                    }
                }
            }
        }

        let wav = bms_rs::bms::model::wav::WavObjects {
            wav_files,
            notes,
            ..Default::default()
        };

        let bms = Self {
            bmp: Default::default(),
            bpm,
            judge: Default::default(),
            metadata: bms_rs::bms::model::metadata::Metadata {
                player: Some(bms_rs::bms::command::PlayerMode::Double),
                play_level: Some(0),
                difficulty: Some(12),
                email: None,
                url: None,
                wav_path_root: None,
                divide_prop: None,
                is_octave: false,
            },
            music_info: bms_rs::bms::model::music_info::MusicInfo {
                genre: Some("Awesome Sauce".into()),
                title: Some("My Song 67".into()),
                subtitle: None,
                artist: Some("John Music".into()),
                sub_artist: None,
                maker: Some("jonnah".into()),
                comment: None,
                preview_music: None,
            },
            option: Default::default(),
            repr: bms_rs::bms::model::repr::BmsSourceRepresentation {
                case_sensitive_obj_id: true,
                ..Default::default()
            },
            resources: Default::default(),
            scroll: Default::default(),
            section_len: Default::default(),
            speed: Default::default(),
            sprite: Default::default(),
            stop: Default::default(),
            text: Default::default(),
            video: Default::default(),
            volume: Default::default(),
            wav,
            randomized: Default::default(),
        };

        Ok(bms)
    }
}
