use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};

pub struct AudioPlayer {
    host: cpal::platform::Host,
    device: cpal::platform::Device,
    sample_rate: crate::project::SampleRate,
    output_streams: Vec<cpal::platform::Stream>,
    config: cpal::StreamConfig,
    audio_files: std::sync::Arc<std::sync::Mutex<Vec<AudioPlayback>>>,
    volume: std::sync::Arc<std::sync::RwLock<f32>>,
}

impl AudioPlayer {
    pub fn new(sample_rate: crate::project::SampleRate) -> Result<Self, crate::Error> {
        let host = cpal::default_host();

        const BUFFER_SIZE_PER_CHANNEL: u32 = 1024;

        let config_for_device = |device: &cpal::Device| {
            device
                .supported_output_configs()?
                .find_map(|output_config| {
                    if output_config.channels() != 2 {
                        return None;
                    }

                    let cpal::SupportedBufferSize::Range { min, max } = output_config.buffer_size()
                    else {
                        return None;
                    };

                    if (*min..=*max).contains(&BUFFER_SIZE_PER_CHANNEL) {
                        // TODO: rebuild audio player on sample rate change
                        output_config.try_with_sample_rate(sample_rate.0.cast_unsigned())
                    } else {
                        None
                    }
                })
                .map(|v| v.config())
                .ok_or(cpal::SupportedStreamConfigsError::DeviceNotAvailable)
        };

        let default_device = host
            .default_output_device()
            .ok_or_else(|| "No audio device available.".to_owned())?;

        let config = if let Ok(config) = config_for_device(&default_device) {
            config
        } else {
            host.output_devices()?
                .find_map(|device| config_for_device(&device).ok())
                .ok_or("no suitable device")?
        };

        let volume = std::sync::Arc::new(std::sync::RwLock::new(1.0));

        let mut player = Self {
            host,
            device: default_device,
            sample_rate,
            output_streams: vec![],
            config: config.clone(),
            audio_files: Default::default(),
            volume: volume.clone(),
        };

        let audio_files_clone = player.audio_files.clone();

        let stream = player
            .device
            .build_output_stream(
                &player.config.clone(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // TODO: Figure out how to acquire this so it's not a const
                    const CHANNEL_COUNT: usize = 2;
                    let num_samples = data.len() / CHANNEL_COUNT;

                    // need to clear existing buffer first
                    for val in data.iter_mut() {
                        *val = 0.0;
                    }

                    let mut ctx = audio_files_clone.lock().expect("Bad mutex");
                    ctx.retain_mut(|playback| {
                        let Some(slices) = playback.read(num_samples) else {
                            return false;
                        };

                        for (i, data) in data.iter_mut().enumerate() {
                            let Some(val) = slices
                                .get(i % CHANNEL_COUNT)
                                .and_then(|v| v.get(i / CHANNEL_COUNT))
                            else {
                                break;
                            };

                            *data += *val;
                        }

                        true
                    });

                    let Ok(volume) = volume.read().map(|v| *v) else {
                        log::error!("failed to fetch audio player volume");
                        return;
                    };

                    for sample in data.iter_mut() {
                        *sample *= volume;
                    }
                },
                move |err| {
                    log::error!("{err}");
                },
                None, // None=blocking, Some(Duration)=timeout
            )
            .expect("Failed to create stream");

        stream.play().expect("failed to play stream");
        player.output_streams.push(stream);

        Ok(player)
    }

    pub fn restart(&self) {
        for output_stream in &self.output_streams {
            output_stream.play().unwrap();
        }
    }

    pub fn add_audio(&self, playback: AudioPlayback) {
        if playback.stream.channels.is_empty() {
            log::error!("empty audio buffer submitted to output stream");
            return;
        }
        if playback.cursor
            >= playback
                .stream
                .channels
                .iter()
                .map(|channel| channel.len())
                .max()
                .unwrap_or(0)
        {
            log::error!("cursor is past playback length");
        }

        self.audio_files
            .lock()
            .expect("Failed to lock audio")
            .push(playback);

        for stream in &self.output_streams {
            stream.play().unwrap();
        }
    }

    pub fn volume(&self) -> f32 {
        *self.volume.read().expect("failed to get volume")
    }

    pub fn set_volume(&self, volume: f32) {
        *self.volume.write().expect("failed to write volume") = volume;
    }

    pub fn sample_rate(&self) -> crate::project::SampleRate {
        self.sample_rate
    }
}

pub struct AudioPlayback {
    stream: std::sync::Arc<AudioStream>,
    length: usize,
    cursor: usize,
}

impl AudioPlayback {
    pub fn new(
        stream: impl Into<AudioStream>,
        cursor_start: Option<usize>,
    ) -> Result<Self, crate::Error> {
        let stream = std::sync::Arc::new(stream.into());

        let length = stream
            .channels
            .iter()
            .map(|channel| channel.len())
            .max()
            .ok_or("no max length? no channels on audio?")?;

        let cursor = cursor_start.unwrap_or(0);
        if cursor > length {
            return Err(format!("cursor start {cursor} is ahead of num samples {length}").into());
        }

        Ok(Self {
            stream,
            length,
            cursor,
        })
    }

    pub fn read(&mut self, num_samples: usize) -> Option<Vec<&[f32]>> {
        let read_size = num_samples.min(self.samples_remaining_per_channel());
        if read_size == 0 {
            return None;
        }

        let start = self.cursor;
        let end = self.cursor + read_size;

        let slices = self
            .stream
            .channels
            .iter()
            .map(|channel| &channel[self.cursor..self.cursor + read_size])
            .collect();

        self.cursor = (self.cursor + read_size).min(self.length);
        Some(slices)
    }

    pub fn samples_remaining_per_channel(&self) -> usize {
        self.length - self.cursor
    }

    pub fn total_samples_remaining(&self) -> usize {
        self.stream.channels.len() * self.samples_remaining_per_channel()
    }
}

pub struct AudioStream {
    channels: Vec<Vec<f32>>,
}

impl From<Vec<Vec<f32>>> for AudioStream {
    fn from(value: Vec<Vec<f32>>) -> Self {
        Self { channels: value }
    }
}

impl AudioStream {
    pub fn new(channels: Vec<Vec<f32>>) -> Self {
        Self { channels }
    }
}
