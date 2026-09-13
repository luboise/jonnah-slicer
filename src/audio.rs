// TODO: Make this part of BPMChange to support other time signatures
pub const BEATS_PER_MEASURE: usize = 4;

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

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct TimePoint {
    pub measure: i64,
    pub submeasure: f64,
}

impl TimePoint {
    pub fn new(measure: i64, submeasure: f64) -> Self {
        let measure = measure + submeasure.trunc() as i64;
        let submeasure = submeasure.fract();

        Self {
            measure,
            submeasure,
        }
        .normalised()
    }

    pub fn from_sample(
        sample: usize,
        sample_rate: i32,
        bpm_changes: &[BPMChange],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_time(sample as f64 / sample_rate as f64, bpm_changes)
    }

    pub fn from_time(
        time_seconds: f64,
        bpm_changes: &[BPMChange],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if bpm_changes.is_empty() {
            return Err("no bpm changes".into());
        }

        if bpm_changes.len() == 1 {
            let beat_length = 60.0 / bpm_changes[0].bpm;
            let num_measures = time_seconds / beat_length / BEATS_PER_MEASURE as f64;

            return Ok(num_measures.into());
        }

        let mut lengths = vec![];

        for bpm_change in bpm_changes {
            lengths.push(calculate_timepoints_distance(
                Default::default(),
                bpm_change.time_point,
                bpm_changes,
            )?);
        }

        let (index_l, time_l, time_r) = if let Some((i, (l, r))) = lengths
            .iter()
            .zip(lengths.iter().skip(1))
            .enumerate()
            .find(|(_, (l, r))| **l <= time_seconds && time_seconds <= **r)
        {
            (i, *l, *r)
        } else {
            (
                lengths.len() - 1,
                lengths.last().copied().ok_or("bad last")?,
                time_seconds,
            )
        };

        // If we are not past the final time point
        if index_l < lengths.len() - 1 {
            let ratio = (time_seconds - time_l) / (time_r - time_l);

            let l = &bpm_changes[index_l];
            let r = &bpm_changes[index_l + 1];

            Ok(l.time_point.ratio(&r.time_point, ratio))
        } else {
            let l = &bpm_changes[index_l];

            let diff = time_r - time_l;
            let measure_length = 60.0 * BEATS_PER_MEASURE as f64 / l.bpm;

            Ok(l.time_point + Self::from(lengths[index_l - 1] + diff / measure_length))
        }
    }

    pub fn clamped_to_zero(self) -> Self {
        if self.measure < 0 {
            Self::default()
        } else {
            self
        }
    }

    pub fn ratio(&self, other: &Self, ratio: impl Into<f64>) -> Self {
        let start = f64::from(*self);
        let end = f64::from(*other);

        Self::from(start + ratio.into() * (end - start))
    }

    pub fn get_ratio(&self, end: &Self, t: &Self) -> f64 {
        let start = f64::from(*self);
        let end = f64::from(*end);
        let t = f64::from(*t);

        (t - start) / (end - start)
    }

    pub fn ceil(&self) -> i64 {
        self.measure + self.submeasure.ceil() as i64
    }

    pub fn seconds_from_start(
        &self,
        bpm_changes: &[BPMChange],
    ) -> Result<f64, Box<dyn std::error::Error>> {
        calculate_timepoints_distance(
            Self {
                measure: 0,
                submeasure: 0.0,
            },
            *self,
            bpm_changes,
        )
    }

    /// Get the sample index of a time point within a given channel.
    pub fn samples_from_start(
        &self,
        channel_sample_rate: i32,
        bpm_changes: &[BPMChange],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let seconds = self.seconds_from_start(bpm_changes)?;

        Ok((seconds * channel_sample_rate as f64) as usize)
    }

    pub fn normalise(&mut self) {
        while self.submeasure < 0.0 {
            self.measure -= 1;
            self.submeasure += 1.0;
        }

        while self.submeasure > 1.0 {
            self.measure += 1;
            self.submeasure -= 1.0;
        }
    }

    pub fn normalised(&self) -> Self {
        let mut ret = *self;
        ret.normalise();
        ret
    }

    pub fn quantise(&mut self, snapping: Snapping) {
        let beat_denom = match snapping {
            Snapping::Measure(v) => f64::from(v),
            Snapping::Beat(v) => f64::from(v) * BEATS_PER_MEASURE as f64,
        };

        let num_divisions = f64::from(*self) * beat_denom;
        *self = (num_divisions.round() / beat_denom).into();
    }

    pub fn quantised(mut self, snapping: Snapping) -> Self {
        self.quantise(snapping);
        self
    }
}

impl PartialOrd for TimePoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.measure.partial_cmp(&other.measure) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.submeasure.partial_cmp(&other.submeasure)
    }
}

impl Eq for TimePoint {}

impl Ord for TimePoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        f64::from(*self)
            .partial_cmp(&f64::from(*other))
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Default for TimePoint {
    fn default() -> Self {
        Self {
            measure: 0,
            submeasure: 0.0,
        }
    }
}

impl From<TimePoint> for f64 {
    fn from(value: TimePoint) -> Self {
        value.measure as Self + value.submeasure
    }
}

impl From<f64> for TimePoint {
    fn from(value: f64) -> Self {
        Self {
            measure: value.trunc() as i64,
            submeasure: value.fract(),
        }
    }
}

impl std::ops::Neg for TimePoint {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            measure: -self.measure,
            submeasure: -self.submeasure,
        }
        .normalised()
    }
}

impl std::ops::Add for TimePoint {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let sub = self.submeasure + rhs.submeasure;

        let measure = self.measure + rhs.measure + sub.trunc() as i64;
        let submeasure = sub.fract();

        Self {
            measure,
            submeasure,
        }
        .normalised()
    }
}

impl std::ops::Sub for TimePoint {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        let mut measure = self.measure - rhs.measure;
        let mut submeasure = self.submeasure - rhs.submeasure;

        measure -= submeasure.abs().ceil() as i64;
        submeasure += submeasure.abs().ceil();

        Self {
            measure,
            submeasure,
        }
        .normalised()
    }
}

impl std::ops::AddAssign for TimePoint {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign for TimePoint {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

#[cfg(test)]
#[path = "./time_point_tests.rs"]
mod time_point_tests;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BPMChange {
    pub time_point: TimePoint,
    pub bpm: f64,
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

    pub fn cuts_from_slices(
        &self,
        slices: &[crate::project::Slice],
        bpm_changes: &[BPMChange],
    ) -> Result<Vec<&[f32]>, Box<dyn std::error::Error>> {
        let sample_counts = std::iter::once(&crate::project::Slice {
            time_point: Default::default(),
        })
        .chain(slices.iter())
        .map(|slice| {
            calculate_num_samples(
                Default::default(),
                slice.time_point,
                self.sample_rate,
                1,
                bpm_changes,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

        let starting_sample = calculate_num_samples(
            Default::default(),
            slices.first().ok_or("no slice 0")?.time_point,
            self.sample_rate,
            1,
            bpm_changes,
        )?;

        let sample_counts = sample_counts
            .clone()
            .into_iter()
            .zip(sample_counts.into_iter().skip(1))
            .map(|(l, r)| r - l)
            .collect::<Vec<_>>();

        self.cuts_from_samples(starting_sample, &sample_counts)
    }

    pub fn cuts_from_samples(
        &self,
        starting_sample: usize,
        frame_counts: &[usize],
    ) -> Result<Vec<&[f32]>, Box<dyn std::error::Error>> {
        let mut cuts = vec![];

        let mut num_samples = 0usize;

        let samples_slice = self.samples.iter().as_slice();

        if starting_sample >= samples_slice.len() {
            return Err(format!("starting sample {starting_sample} out of range").into());
        }

        let samples_slice = &samples_slice[starting_sample..];

        for (i, num_frames) in frame_counts.iter().enumerate() {
            let num_to_read = num_frames * usize::from(self.num_channels());
            let Ok(cut) = samples_slice
                .get(num_samples..num_samples + num_to_read)
                .ok_or_else(|| {
                    format!(
                        "unable to get samples[{num_samples}..{}] ({} available)",
                        num_samples + num_to_read,
                        samples_slice.len()
                    )
                })
            else {
                eprintln!(
                    "not enough samples to fulfill all slices, dropped {}",
                    frame_counts.len() - i
                );
                break;
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

    pub fn export_slices(
        &self,
        export_dir: impl AsRef<std::path::Path>,
        slices: &crate::project::Slices,
        bpm_changes: &[BPMChange],
        file_name_fn: Option<impl Fn(usize) -> String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cuts = self.cuts_from_slices(&slices.0, bpm_changes)?;

        let export_dir = export_dir.as_ref();

        if !export_dir.exists() {
            std::fs::create_dir_all(export_dir)?;
        }

        for (i, cut) in cuts.into_iter().enumerate() {
            let file_stem = file_name_fn
                .as_ref()
                .map(|f| f(i))
                .unwrap_or(format!("{i:0>2}"));

            let file_name = file_stem + ".wav";

            if let Err(e) = wavers::write(export_dir.join(&file_name), cut, self.sample_rate(), 2) {
                return Err(format!("failed to export stem {}: {e}", file_name).into());
            }
        }

        Ok(())
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
            let ratio = (i as f64) / ((num_points - 1) as f64);

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
    start: TimePoint,
    end: TimePoint,
    bpm_changes: &[BPMChange],
) -> Result<f64, Box<dyn std::error::Error>> {
    let prefirst_bpm_change = bpm_changes
        .iter()
        .position(|bpm_change| bpm_change.time_point <= start)
        .ok_or("no start time point")?;

    let final_bpm_change = bpm_changes
        .iter()
        .rposition(|bpm_change| end >= bpm_change.time_point)
        .ok_or("no end time point")?;

    let first = [BPMChange {
        time_point: start,
        bpm: bpm_changes[prefirst_bpm_change].bpm,
    }];

    let last = [BPMChange {
        time_point: end,
        bpm: bpm_changes[final_bpm_change].bpm,
    }];

    let bpm_changes = (first.iter())
        .chain(&bpm_changes[prefirst_bpm_change + 1..=final_bpm_change])
        .chain(&last);

    let bpm_changes = bpm_changes.clone().zip(bpm_changes.skip(1));

    Ok(bpm_changes.fold(0.0, |acc, (bpm_change1, bpm_change2)| {
        acc + f64::from(bpm_change2.time_point - bpm_change1.time_point) * 60.0 / bpm_change1.bpm
            * BEATS_PER_MEASURE as f64
    }))
}

pub fn calculate_num_samples(
    start: TimePoint,
    end: TimePoint,
    sample_rate: crate::project::SampleRate,
    num_channels: u16,
    bpm_changes: &[BPMChange],
) -> Result<usize, Box<dyn std::error::Error>> {
    let num_seconds = calculate_timepoints_distance(start, end, bpm_changes)?;
    let samples_per_second = sample_rate.0 as usize * num_channels as usize;

    Ok((samples_per_second as f64 * num_seconds).ceil() as usize)
}

#[path = "audio_tests.rs"]
#[cfg(test)]
mod tests;
