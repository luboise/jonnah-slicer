#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct Slice {
    pub time_point: crate::audio::TimePoint,
    // Room here later to add de-duplication of keysounds and custom keysound IDs
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
