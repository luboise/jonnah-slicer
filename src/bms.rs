impl TryFrom<crate::audio::TimePoint> for bms_rs::bms::command::time::ObjTime {
    type Error = crate::Error;

    fn try_from(value: crate::audio::TimePoint) -> Result<Self, Self::Error> {
        // TODO: Make time points store the values based on fractions instead
        let numerator = (((value.submeasure) * 4.0) as u64) + 1;
        let denominator = 4;

        let time =
            bms_rs::bms::command::time::ObjTime::new(value.measure as u64, numerator, denominator)
                .ok_or_else(|| format!("failed to create ObjTime from TimePoint {value:?}"))?;

        Ok(time)
    }
}

impl TryFrom<crate::audio::BPMChange> for bms_rs::bms::model::obj::BpmChangeObj {
    type Error = crate::Error;

    fn try_from(value: crate::audio::BPMChange) -> Result<Self, Self::Error> {
        let crate::audio::BPMChange { time_point, bpm } = value;

        Ok(Self {
            time: time_point.try_into()?,
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
            bpm_changes,
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

        for (channel, stem) in stems.iter().enumerate() {
            let note_channel_id = [b'1', base62::encode(channel as u64 + 1).as_bytes()[0]]
                .try_into()
                .map_err(|e| format!("bad input: {e:#?}"))?;

            let obj_id = stem.starting_keysound.unwrap_or(0);

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

            for (i, slice) in stem.slices.iter().enumerate() {
                let wav_path = format!("{file_stem}_{i:03}.{file_extension}");
                let mut encoded = base62::encode(obj_id + i as u64);
                if encoded.len() == 1 {
                    encoded.insert(0, '0');
                }

                let wav_obj_id = bms_rs::bms::command::ObjId::try_from(&encoded, true)?;
                wav_files.insert(wav_obj_id, wav_path.into());

                // notes.push_bgm::<bms_rs::bms::command::channel::mapper::KeyLayoutBeat>(
                //     slice.time_point.try_into()?,
                //     wav_obj_id,
                // );

                notes.push_note(bms_rs::bms::model::obj::WavObj {
                    offset: slice.time_point.try_into()?,
                    channel_id: note_channel_id,
                    wav_id: wav_obj_id,
                });
            }
        }

        let wav = bms_rs::bms::model::wav::WavObjects {
            wav_files,
            notes,
            ..Default::default()
        };

        let bms = bms_rs::bms::model::Bms {
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
