use crate::audio::RatioExt as _;

impl TryFrom<crate::audio::BPMChange> for bms_rs::bms::model::obj::BpmChangeObj {
    type Error = crate::Error;

    fn try_from(value: crate::audio::BPMChange) -> Result<Self, Self::Error> {
        let crate::audio::BPMChange { time_point, bpm } = value;

        Ok(Self {
            time: time_point.to_objtime()?,
            bpm: bpm.try_into()?,
        })
    }
}

impl TryFrom<crate::project::Project> for bms_rs::bms::model::Bms {
    type Error = crate::Error;

    fn try_from(value: crate::project::Project) -> Result<Self, Self::Error> {
        let crate::project::Project {
            sample_rate: _,
            stems,
            timing: bpm_changes,
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

        let mut num_note_channels = 0;

        let mut wav_files = std::collections::HashMap::new();
        let mut notes = bms_rs::bms::model::Notes::default();

        for stem in stems {
            // base62 keysounds start at 01 not 00
            let obj_id = stem.starting_keysound.unwrap_or(1);

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

            let mut obj_ids = vec![];

            for (slice_i, slice) in stem.slices.0.iter().enumerate() {
                if matches!(slice.keysound_id, crate::project::SliceKeysound::Auto) {
                    // otherwise, it's a new keysound. insert it
                    let mut encoded = base62::encode(obj_id + obj_ids.len() as u64);
                    if encoded.len() == 1 {
                        encoded.insert(0, '0');
                    }

                    let wav_path = format!("{file_stem}_{:03}.{file_extension}", obj_ids.len());
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
                    crate::project::StemType::Note if num_note_channels < 8 => {
                        let note_channel_id = [b'1', b'1' + num_note_channels]
                            .try_into()
                            .map_err(|e| format!("bad input: {e:#?}"))?;

                        notes.push_note(bms_rs::bms::model::obj::WavObj {
                            offset: slice.time_point.to_objtime()?,
                            channel_id: note_channel_id,
                            wav_id: wav_obj_id,
                        });
                    }
                    crate::project::StemType::Note | crate::project::StemType::BGM => {
                        notes.push_bgm::<bms_rs::bms::command::channel::mapper::KeyLayoutBeat>(
                            slice.time_point.to_objtime()?,
                            wav_obj_id,
                        );
                    }
                }
            }

            if matches!(stem.ty, crate::project::StemType::Note) {
                num_note_channels = num_note_channels.saturating_add(1);
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
                player: Some(bms_rs::bms::command::PlayerMode::Single),
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
