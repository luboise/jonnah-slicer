// TODO: Make this part of BPMChange to support other time signatures
pub const BEATS_PER_MEASURE: usize = 4;

/// Get slices to feed to [`AudioFile::cuts_from_sample_counts`]
pub fn slices_to_sample_counts(
    sample_rate: crate::project::SampleRate,
    num_channels: u16,
    slices: &[crate::project::Slice],
    bpm_changes: &Timing,
    num_samples: usize,
) -> Result<(usize, Vec<usize>), crate::Error> {
    let sample_counts = slices
        .iter()
        .map(|slice| {
            calculate_num_samples(
                Default::default(),
                slice.time_point,
                sample_rate,
                num_channels,
                bpm_changes,
            )
        })
        // Add on a fake one at the end equal to the end of the file
        .chain(std::iter::once(Ok(num_samples)))
        .collect::<Result<Vec<_>, _>>()?;

    let starting_sample = calculate_num_samples(
        TimePoint::default(),
        slices.first().ok_or("no slice 0")?.time_point,
        sample_rate,
        1,
        bpm_changes,
    )?;

    let sample_counts = sample_counts
        .clone()
        .into_iter()
        .zip(sample_counts.into_iter().skip(1))
        .map(|(l, r)| r.checked_sub(l).ok_or("bad sub in sample counts"))
        .collect::<Result<Vec<_>, _>>()?;

    Ok((starting_sample, sample_counts))
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Snapping {
    Measure(u16),
    Beat(u16),
}

impl Snapping {
    pub fn simplify(self) -> Self {
        match self {
            Self::Measure(m) => Self::Beat(m * BEATS_PER_MEASURE as u16),
            Self::Beat(b) => Self::Beat(b),
        }
    }

    pub fn as_measure_denom(self) -> u16 {
        match self {
            Self::Measure(m) => m,
            Self::Beat(b) => b * BEATS_PER_MEASURE as u16,
        }
    }
}

impl Default for Snapping {
    fn default() -> Self {
        Self::Measure(16)
    }
}

pub type TimePoint = num_rational::Ratio<i64>;
pub trait RatioExt: Sized {
    fn from_measure(measure: i64) -> Self;
    fn from_submeasure(measure: i64, numerator: i64, denominator: i64) -> Self;
    fn from_sample(
        sample: usize,
        sample_rate: i32,
        bpm_changes: &Timing,
    ) -> Result<Self, crate::Error> {
        let ms = ((sample as f64 / sample_rate as f64) * 1_000_000.0) as u64;
        Self::from_time(ms, bpm_changes)
    }

    fn from_time(time_microseconds: u64, bpm_changes: &Timing) -> Result<Self, crate::Error>;

    fn clamped_to_zero(self) -> Self;
    fn ratio(&self, other: &Self, ratio: impl Into<Self>) -> Self;

    fn quantise(&mut self, snapping: Snapping);
    fn quantised(mut self, snapping: Snapping) -> Self {
        self.quantise(snapping);
        self
    }

    fn ratio_at_t(&self, end: &Self, t: &Self) -> Self;

    fn seconds_from_start(&self, bpm_changes: &Timing) -> Result<f64, crate::Error>;
    fn samples_from_start(
        &self,
        channel_sample_rate: i32,
        bpm_changes: &Timing,
    ) -> Result<usize, crate::Error>;

    fn abs(&self) -> Self;

    fn to_f32(&self) -> f32;
    fn to_f64(&self) -> f64;
    fn to_objtime(&self) -> Result<bms_rs::bms::command::time::ObjTime, crate::Error>;
}

impl RatioExt for TimePoint {
    fn from_measure(measure: i64) -> Self {
        Self::from_integer(measure)
    }

    fn from_submeasure(measure: i64, numerator: i64, denominator: i64) -> Self {
        let ratio = Self::new(numerator, denominator);
        ratio + Self::from_integer(measure)
    }

    fn from_time(time_microseconds: u64, bpm_changes: &Timing) -> Result<Self, crate::Error> {
        let x = bpm_changes.get_timepoints(&[time_microseconds]);
        Ok(x[0])
    }

    fn clamped_to_zero(self) -> Self {
        if self <= Self::ZERO { Self::ZERO } else { self }
    }

    fn ratio(&self, other: &Self, ratio: impl Into<Self>) -> Self {
        let diff = other - self;

        Self::from(self + ratio.into() * (diff))
    }

    fn quantise(&mut self, snapping: Snapping) {
        let denom: i64 = match snapping {
            Snapping::Measure(v) => v.into(),
            Snapping::Beat(v) => (v * BEATS_PER_MEASURE as u16).into(),
        };

        *self = (*self * denom).round() / denom;
    }

    fn ratio_at_t(&self, end: &Self, t: &Self) -> Self {
        let start = self;
        let end = end;

        (t - start) / (end - start)
    }

    fn seconds_from_start(&self, bpm_changes: &Timing) -> Result<f64, crate::Error> {
        calculate_timepoints_distance(Self::ZERO, *self, bpm_changes)
    }

    /// Get the sample index of a time point within a given channel.
    fn samples_from_start(
        &self,
        channel_sample_rate: i32,
        timing: &Timing,
    ) -> Result<usize, crate::Error> {
        let seconds = self.seconds_from_start(timing)?;

        Ok((seconds * channel_sample_rate as f64) as usize)
    }

    fn to_f32(&self) -> f32 {
        *self.numer() as f32 / *self.denom() as f32
    }

    fn to_f64(&self) -> f64 {
        *self.numer() as f64 / *self.denom() as f64
    }

    // pub fn ceil(&self) -> i64 {
    //     self.0.ceil().to_integer()
    // }

    fn to_objtime(&self) -> Result<bms_rs::bms::command::time::ObjTime, crate::Error> {
        let measure = self.floor().to_integer();
        let (n, d) = self.fract().into_raw();

        Ok(bms_rs::bms::command::time::ObjTime::new(
            measure.try_into()?,
            n.try_into()?,
            d.try_into()?,
        )
        .ok_or("bad objtime")?)
    }

    fn abs(&self) -> Self {
        if Self::ZERO < *self {
            -self.clone()
        } else {
            self.clone()
        }
    }
}

#[cfg(test)]
#[path = "./time_point_tests.rs"]
mod time_point_tests;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct Timing {
    bpm_changes: Vec<BPMChange>,
}

pub type BPM = f64;

impl Default for Timing {
    fn default() -> Self {
        Self {
            bpm_changes: vec![BPMChange {
                time_point: Default::default(),
                bpm: 120.0,
            }],
        }
    }
}

impl Timing {
    pub fn base_bpm(&self) -> BPM {
        self.bpm_changes[0].bpm
    }

    pub fn bpm_changes(&self) -> &[BPMChange] {
        &self.bpm_changes
    }

    pub fn iter(&self) -> std::slice::Iter<'_, BPMChange> {
        self.bpm_changes.iter()
    }

    pub fn first(&self) -> Option<&BPMChange> {
        self.bpm_changes.first()
    }

    pub fn last(&self) -> Option<&BPMChange> {
        self.bpm_changes.last()
    }

    pub fn get_bpm_change_unchecked(&self, i: usize) -> BPMChange {
        self.bpm_changes[i]
    }

    /// Get the position of one (or many) time points in a given set of timings, as a duration from the start of the song
    pub fn times_of(&self, time_points: &[TimePoint]) -> Vec<std::time::Duration> {
        if time_points.is_empty() {
            return vec![];
        }

        let mut acc: BPM = 0.0;

        let mut durations = vec![None; time_points.len()];

        'outer: for [l, r] in self.bpm_changes.array_windows().chain(
            [
                self.bpm_changes.last().unwrap().clone(),
                BPMChange {
                    time_point: TimePoint::from_integer(i64::MAX),
                    bpm: 120.0,
                },
            ]
            .array_windows(),
        ) {
            for (i, tp) in time_points.iter().copied().enumerate() {
                // skip ones we've already found
                if durations[i].is_some() {
                    continue;
                }

                // ignore time points out of range
                if tp < l.time_point || tp > r.time_point {
                    continue;
                }

                if l.time_point <= tp && tp <= r.time_point {
                    let num_beats = (tp - l.time_point) * (BEATS_PER_MEASURE as i64);
                    let beat_length = 60.0 / l.bpm;

                    let diff_dur = num_beats.to_f64() * beat_length;

                    durations[i] = Some(std::time::Duration::from_secs_f64(acc + diff_dur));

                    if durations.iter().all(Option::is_some) {
                        break 'outer;
                    }
                }
            }

            let beat_length = 60.0 / l.bpm;
            acc += beat_length * (r.time_point - l.time_point).to_f64() * BEATS_PER_MEASURE as f64;
        }

        assert!(
            durations.iter().all(Option::is_some),
            "some durations weren't findable"
        );
        durations.into_iter().map(|v| v.unwrap()).collect()
    }

    pub fn get_timepoints(&self, times_microseconds: &[u64]) -> Vec<TimePoint> {
        if times_microseconds.is_empty() {
            return vec![];
        }

        let mut acc = std::time::Duration::from_secs(0);

        let mut times_microseconds = times_microseconds.to_vec();
        let mut time_points = vec![None; times_microseconds.len()];

        'outer: for [l, r] in self.bpm_changes.array_windows().chain(
            [
                self.bpm_changes.last().copied().unwrap(),
                BPMChange {
                    time_point: TimePoint::from_integer(i64::MAX),
                    bpm: 120.0,
                },
            ]
            .array_windows(),
        ) {
            let segment_duration = if *r.time_point.numer() == i64::MAX {
                std::time::Duration::from_micros(i64::MAX as u64)
            } else {
                let diff_measures = (r.time_point - l.time_point).to_f64();
                let diff_beats = diff_measures * BEATS_PER_MEASURE as f64;
                std::time::Duration::from_secs_f64(diff_beats * (60.0 / l.bpm))
            };

            for (i, time_μs) in times_microseconds.iter_mut().enumerate() {
                // skip ones we've already found
                if time_points[i].is_some() {
                    continue;
                }

                let time_ms_duration = std::time::Duration::from_micros(*time_μs);
                // skip ones which aren't due yet
                if time_ms_duration > segment_duration {
                    *time_μs -= segment_duration.as_micros() as u64;
                    continue;
                }

                let diff_μs = *time_μs as f64;
                let beat_length = 60_000_000.0 / l.bpm;

                let mut num_beats = diff_μs / beat_length;

                let mut num_measures = 0;
                while num_beats >= BEATS_PER_MEASURE as f64 {
                    num_measures += 1;
                    num_beats -= BEATS_PER_MEASURE as f64;
                }

                time_points[i] = Some(
                    l.time_point
                        + TimePoint::from_submeasure(
                            num_measures,
                            // TODO: Fix this rounding here
                            ((num_beats / BEATS_PER_MEASURE as f64) * 1000.0) as i64,
                            1000,
                        ),
                );

                if time_points.iter().all(Option::is_some) {
                    break 'outer;
                }
            }

            // This diff duration will fail on unchecked mul if not careful
            let diff_duration = {
                let diff_measures = r.time_point - l.time_point;
                let diff_beats = diff_measures * TimePoint::from_integer(BEATS_PER_MEASURE as i64);

                let beat_length = 60.000 / l.bpm;

                std::time::Duration::from_secs_f64(diff_beats.to_f64() * beat_length)
            };
            acc += diff_duration;
        }

        time_points.into_iter().map(Option::unwrap).collect()
    }
}

impl IntoIterator for Timing {
    type Item = BPMChange;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.bpm_changes.into_iter()
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BPMChange {
    pub time_point: TimePoint,
    pub bpm: BPM,
}

#[derive(Debug)]
pub struct AudioFile {
    sample_rate: crate::project::SampleRate,
    wav: wavers::Wav<f32>,
    samples: wavers::Samples<f32>,
}

impl AudioFile {
    pub fn new(mut wav: wavers::Wav<f32>) -> Result<Self, wavers::WaversError> {
        let samples = wav.read()?;
        Ok(Self {
            sample_rate: crate::project::SampleRate(wav.sample_rate()),
            wav,
            samples,
        })
    }

    pub fn sample_rate(&self) -> i32 {
        self.wav.sample_rate()
    }

    pub fn num_channels(&self) -> u16 {
        self.wav.n_channels()
    }

    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }

    pub fn num_samples_per_channel(&self) -> usize {
        self.num_samples() / self.num_channels() as usize
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn channels(&self) -> Vec<Vec<f32>> {
        let mut vecs = vec![vec![]; self.num_channels().into()];

        for (i, sample) in self.samples.iter().enumerate() {
            vecs[i % usize::from(self.num_channels())].push(*sample);
        }

        vecs
    }

    pub fn cuts_from_sample_counts(
        &self,
        starting_sample: usize,
        frame_counts: &[usize],
    ) -> Result<Vec<&[f32]>, crate::Error> {
        let mut cuts = vec![];

        let mut num_samples = 0usize;

        let samples_slice = self.samples.iter().as_slice();

        let starting_sample = starting_sample * self.num_channels() as usize;

        if starting_sample >= samples_slice.len() {
            return Err(format!("starting sample {starting_sample} out of range").into());
        }

        let samples_slice = &samples_slice[starting_sample..];

        for (i, num_frames) in frame_counts.iter().enumerate() {
            let num_to_read = num_frames * usize::from(self.num_channels());

            let cut = match samples_slice
                .get(num_samples..num_samples + num_to_read)
                .ok_or_else(|| {
                    format!(
                        "unable to get samples[{num_samples}..{}] ({} available)",
                        num_samples + num_to_read,
                        samples_slice.len()
                    )
                }) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "not enough samples to fulfill all slices, dropped {}: {e}",
                        frame_counts.len() - i
                    );
                    break;
                }
            };

            num_samples += num_to_read;

            if cut.len() < *num_frames {
                return Err(format!(
                    "less frames ({}) than expected {num_frames} ({} available)",
                    cut.len(),
                    samples_slice.len()
                )
                .into());
            }

            cuts.push(cut);
        }

        Ok(cuts)
    }

    pub fn draw_channel(
        &self,
        channel_index: u16,
        num_points: Option<usize>,
        starting_sample: usize,
        num_samples: usize,
        rect: &egui::Rect,
        painter: &egui::Painter,
        stroke: egui::Stroke,
    ) {
        // Clamp to a normal amount
        let num_points = num_points
            .map(|v| v.min(self.num_samples_per_channel()))
            .unwrap_or_else(|| self.samples.len());

        let samples = (0..num_points).map(|i| {
            let ratio = (i as BPM) / ((num_points - 1) as BPM);

            let index = (ratio * (num_samples as f64)) as usize + starting_sample;

            // Adjust for channel sample
            self.samples
                .get(index * usize::from(self.num_channels()) + usize::from(channel_index))
        });

        let points = samples
            .into_iter()
            .take(num_points)
            .enumerate()
            .filter_map(|(i, sample)| {
                let sample = sample?;

                let tx = i as f64 / (num_points - 1) as f64;

                // [-1, 1] -> [0, 1], then invert for egui Y downwards
                let ty = 1.0 - f32::midpoint(*sample, 1.0);
                Some(egui::pos2(
                    rect.min.x + tx as f32 * rect.width(),
                    rect.min.y + ty * rect.height(),
                ))
            })
            .collect::<Vec<_>>();

        if points.len() > 1 {
            painter.line(points, stroke);
        }
    }
}

pub fn calculate_timepoints_distance(
    start: impl Into<TimePoint>,
    end: impl Into<TimePoint>,
    timing: &Timing,
) -> Result<f64, crate::Error> {
    let (start, end) = {
        let start = start.into();
        let end = end.into();

        if start < end {
            (start, end)
        } else {
            (end, start)
        }
    };

    let times = timing.times_of(&[start, end]);

    let l = times[0];
    let r = times[1];

    Ok((r - l).as_secs_f64())
}

pub fn export_stem(
    export_dir: impl AsRef<std::path::Path>,
    stem_prefix: &str,
    audio: &[&AudioFile],
    slices: &crate::project::Slices,
    timing: &Timing,
) -> Result<(), crate::Error> {
    let export_dir = export_dir.as_ref();

    for audio in audio {
        let (starting_sample, cuts) = slices_to_sample_counts(
            audio.sample_rate().into(),
            // TODO: Use proper number of channels here, this was hardcoded to 1 before but I can't
            // remember why
            1, // audio.num_channels(),
            &slices.0,
            timing,
            audio.num_samples_per_channel(),
        )?;
        let cuts = audio.cuts_from_sample_counts(starting_sample, &cuts)?;

        if !export_dir.exists() {
            std::fs::create_dir_all(export_dir)?;
        }

        for (i, cut) in cuts.into_iter().enumerate() {
            let file_name = format!("{stem_prefix}_{i:03}.wav");

            if let Err(e) = wavers::write(export_dir.join(&file_name), cut, audio.sample_rate(), 2)
            {
                return Err(format!("failed to export stem {file_name}: {e}").into());
            }
        }
    }

    Ok(())
}

pub fn calculate_num_samples(
    start: TimePoint,
    end: TimePoint,
    sample_rate: crate::project::SampleRate,
    num_channels: u16,
    timing: &Timing,
) -> Result<usize, crate::Error> {
    let num_seconds = calculate_timepoints_distance(start, end, timing)?;
    let samples_per_second = sample_rate.0 as usize * num_channels as usize;

    Ok((samples_per_second as f64 * num_seconds).ceil() as usize)
}

#[path = "audio_tests.rs"]
#[cfg(test)]
mod tests;
