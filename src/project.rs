use crate::audio::RatioExt as _;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub enum SliceKeysound {
    #[default]
    Auto,
    /// Reference to a keysound at a specific index
    Reference(crate::audio::TimePoint),
    // Absolute(u64), // TODO: Implement absolute keysounding
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub time_point: crate::audio::TimePoint,
    #[serde(default)]
    pub keysound_id: SliceKeysound,
}

impl Slice {
    pub fn new(time_point: crate::audio::TimePoint) -> Self {
        Self {
            time_point,
            keysound_id: SliceKeysound::default(),
        }
    }
}

impl PartialOrd for Slice {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let Self {
            time_point,
            keysound_id: _,
        } = self;

        time_point.partial_cmp(&other.time_point)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(transparent)]
pub struct Slices(pub Vec<Slice>);

impl IntoIterator for Slices {
    type Item = Slice;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Slices {
    pub fn slices(&self) -> &[Slice] {
        &self.0
    }

    pub fn insert(&mut self, new_slice: Slice) {
        if self.0.iter().any(|v| v.time_point == new_slice.time_point) {
            return;
        }
        // TODO: Make this not redo the entire thing on every insert
        self.0.push(new_slice);
        self.0.sort_by_key(|f| f.time_point);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Slice> {
        self.0.iter()
    }

    pub fn union(&mut self, other: &Self) {
        for slice in other.iter() {
            self.insert(slice.clone());
        }
    }

    /// get the slice corresponding with a specific timepoint
    pub fn query(&self, time: crate::audio::TimePoint) -> Option<(usize, &Slice)> {
        self.0
            .iter()
            .enumerate()
            .find(|(_, slice)| slice.time_point == time)
    }

    /// query for a timepoint, following all references back to the original slice
    pub fn query_dereferenced(&self, ref_tp: crate::audio::TimePoint) -> Option<&Slice> {
        let mut depth = 0;

        let (_, mut slice) = self.query(ref_tp)?;

        while depth < 5
            && let SliceKeysound::Reference(ref_tp) = slice.keysound_id
        {
            slice = self.query(ref_tp)?.1;
            depth += 1;
        }

        if depth >= 5 {
            log::error!("infinite loop de-referencing time point {ref_tp}");
            return None;
        }

        Some(slice)
    }

    pub fn keysounds(&self) -> impl Iterator<Item = &Slice> {
        self.0
            .iter()
            .filter(|slice| matches!(slice.keysound_id, SliceKeysound::Auto))
    }

    pub fn keysounds_required(&self) -> usize {
        self.keysounds().count()
    }

    pub fn keysound_index_of(&self, index: usize) -> Option<usize> {
        let slice = self.0.get(index)?;

        match slice.keysound_id {
            SliceKeysound::Auto => Some(
                self.0[..index]
                    .iter()
                    .filter(|v| matches!(v.keysound_id, SliceKeysound::Auto))
                    .count(),
            ),
            SliceKeysound::Reference(ref_tp) => self
                .keysounds()
                .position(|slice| slice.time_point == ref_tp),
        }
    }

    /// get n'th slice which is NOT a reference
    ///
    /// Used for resolving which slice a reference is pointing to
    pub fn query_referenced(&self, ref_index: usize) -> Option<(usize, &Slice)> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, slice)| match slice.keysound_id {
                SliceKeysound::Auto => true,
                SliceKeysound::Reference(_) => false,
            })
            .nth(ref_index)
    }

    pub fn query_range(
        &self,
        range: std::range::RangeInclusive<crate::audio::TimePoint>,
    ) -> Vec<&Slice> {
        self.0
            .iter()
            .filter(|slice| range.contains(&slice.time_point))
            .collect()
    }

    /// Get the closest slice to a specific time point (in measures)
    ///
    /// Returns (slice, ABS(distance)) if found
    pub fn closest(
        &self,
        time_point: crate::audio::TimePoint,
    ) -> Option<(&Slice, crate::audio::TimePoint)> {
        log::trace!("getting closest slice to {time_point:?}");

        self.0
            .iter()
            .map(|slice| (slice, (slice.time_point - time_point).abs()))
            .min_by_key(|(_, distance)| *distance)
    }

    /// [`Self::closest`] but mut ref to slice
    pub fn closest_mut(
        &mut self,
        time_point: crate::audio::TimePoint,
    ) -> Option<(&mut Slice, crate::audio::TimePoint)> {
        self.0
            .iter_mut()
            .map(|slice| {
                // copy before moving &mut slice
                let tp = slice.time_point;
                (slice, (tp - time_point).abs())
            })
            .min_by_key(|(_, distance)| *distance)
    }

    /// Get the closest slice to a specific time point (in measures), which is <= distance apart
    ///
    /// This prevents closest from returning a value which is too far away
    ///
    /// Returns (slice, ABS(distance)) if found
    pub fn closest_bound(
        &self,
        time_point: crate::audio::TimePoint,
        measures_bound: f64,
    ) -> Option<(&Slice, crate::audio::TimePoint)> {
        self.closest(time_point)
            .filter(|(_, d)| d.to_f64() <= measures_bound)
    }

    /// [`Self::closest_bound`] but mut ref to slice
    pub fn closest_bound_mut(
        &mut self,
        time_point: crate::audio::TimePoint,
        measures_bound: f64,
    ) -> Option<(&mut Slice, crate::audio::TimePoint)> {
        self.closest_mut(time_point)
            .filter(|(_, d)| d.to_f64() <= measures_bound)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone, Copy)]
pub enum StemType {
    Note,
    #[default]
    BGM,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Stem {
    pub audio_path: std::path::PathBuf,
    pub slices: Slices,
    pub starting_keysound: Option<u64>,
    #[serde(default)]
    pub ty: StemType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl Stem {
    pub fn from_audio_path(audio_path: impl AsRef<std::path::Path>) -> Self {
        let audio_path = audio_path.as_ref().to_owned();

        Self {
            audio_path,
            slices: crate::project::Slices::default(),
            starting_keysound: None,
            ty: crate::project::StemType::default(),
            group: None,
        }
    }

    pub fn slices(&self) -> &Slices {
        &self.slices
    }

    pub fn slices_mut(&mut self) -> &mut Slices {
        &mut self.slices
    }

    pub fn export_prefix(&self) -> Option<&str> {
        if let Some(group) = &self.group {
            Some(group.as_str())
        } else {
            let v = self.audio_path.file_stem()?;
            v.to_str()
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct SampleRate(pub i32);

impl From<i32> for SampleRate {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl egui::emath::Numeric for SampleRate {
    const INTEGRAL: bool = true;

    const MIN: Self = Self(0);

    const MAX: Self = Self(192_000);

    fn to_f64(self) -> f64 {
        self.0.into()
    }

    fn from_f64(num: f64) -> Self {
        Self(num.trunc() as i32)
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        Self(44100)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Project {
    #[serde(default)]
    pub sample_rate: SampleRate,
    pub stems: Vec<Stem>,
    pub timing: crate::audio::Timing,
    #[serde(default)]
    pub samples_fadeout: u64,
}

pub fn normalise_project_path(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    let path = path.as_ref();

    if path.is_dir() {
        path.join("project.jonnah")
    } else {
        path.to_path_buf()
    }
}

pub fn load_project(path: impl AsRef<std::path::Path>) -> Result<Project, crate::Error> {
    let load_path = normalise_project_path(path.as_ref());

    let project =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(load_path)?))?;

    Ok(project)
}

pub fn save_project(
    project: &Project,
    path: impl AsRef<std::path::Path>,
) -> Result<(), crate::Error> {
    let save_path = normalise_project_path(path.as_ref());

    let parent = save_path.parent().ok_or("unable to get parent path")?;

    if !parent.exists() {
        log::info!("creating project dir {}", parent.display());
        std::fs::create_dir_all(parent)?;
    }

    log::info!("saving project to {}", save_path.display());
    serde_json::to_writer_pretty(
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| e.to_string())?,
        &project,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn calculate_initial_keysounds(
    stems: Vec<&mut Stem>,
    wiggle_room: impl Into<u64>,
) -> Result<(), crate::Error> {
    let wiggle_room = wiggle_room.into();

    let mut sorted = std::collections::HashMap::new();

    let default_key = "".to_owned();

    for stem in stems {
        let group = stem.group.clone().unwrap_or_else(|| default_key.clone());
        sorted.entry(group).or_insert_with(Vec::new).push(stem);
    }

    let strays = sorted.remove(&default_key).unwrap_or_default();

    let mut keysound = 1;

    #[expect(
        clippy::iter_over_hash_type,
        reason = "this won't ever run on a redundant system?"
    )]
    for (group_name, stems) in sorted {
        let slices = stems.iter().fold(Slices::default(), |mut acc, stem| {
            acc.union(&stem.slices);
            acc
        });

        if slices.0.is_empty() {
            continue;
        }

        for stem in stems {
            stem.starting_keysound = Some(keysound);
        }

        let num_keysounds: u64 = slices.keysounds_required().try_into()?;
        keysound = keysound
            .checked_add(num_keysounds + wiggle_room)
            .ok_or("slice count overflow")?;
    }

    for stray in strays {
        if stray.slices.0.is_empty() {
            continue;
        }

        stray.starting_keysound = Some(keysound);

        let num_keysounds: u64 = stray.slices.keysounds_required().try_into()?;
        keysound = keysound
            .checked_add(num_keysounds + wiggle_room)
            .ok_or("slice count overflow")?;
    }

    if keysound >= 62 * 62 {
        return Err(format!(
            "too many slices! {keysound} (max number is {})",
            (62 * 62) - 1,
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_keysounds() {
        use crate::audio::TimePoint;
        use SliceKeysound::{Auto, Reference};

        let auto = |m| Slice {
            time_point: TimePoint::from_measure(m),
            keysound_id: Auto,
        };

        let reference = |m, r| Slice {
            time_point: TimePoint::from_measure(m),
            keysound_id: Reference(TimePoint::from_integer(r)),
        };

        let slices = Slices(vec![
            auto(0),
            auto(1),
            reference(2, 0),
            auto(3),
            reference(4, 1),
        ]);

        assert_eq!(slices.keysound_index_of(0), Some(0));
        assert_eq!(slices.keysound_index_of(1), Some(1));
        assert_eq!(slices.keysound_index_of(2), Some(0));
        assert_eq!(slices.keysound_index_of(3), Some(2));
        assert_eq!(slices.keysound_index_of(4), Some(1));
    }
}
