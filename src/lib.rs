mod app;
mod audio;
mod audio_player;
mod bms;
pub mod logging;
pub mod project;

pub use app::JonnahSlicer;

use audio::RatioExt as _;

pub type Error = Box<dyn std::error::Error>;

pub(crate) fn slices_from_midi(
    bytes: &[u8],
    timing: &audio::Timing,
) -> Result<Vec<project::Slice>, Error> {
    use midi_reader_writer::midly_0_5::exports::{MidiMessage, TrackEventKind};

    let midi_file = midi_reader_writer::midly_0_5::exports::Smf::parse(bytes)?;

    let mut ticks_to_micros =
        midi_reader_writer::ConvertTicksToMicroseconds::try_from(midi_file.header)?;

    let mut slices = vec![];

    for (ticks, _track_index, event) in
        midi_reader_writer::midly_0_5::merge_tracks(&midi_file.tracks)
    {
        let microseconds = ticks_to_micros.convert(ticks, &event);

        match event {
            TrackEventKind::SysEx(..) | TrackEventKind::Escape(..) | TrackEventKind::Meta(..) => (),
            TrackEventKind::Midi {
                channel: _,
                message,
            } => match message {
                MidiMessage::NoteOn { key: _, vel: _ } => {
                    let tp = audio::TimePoint::from_time(microseconds, timing)?;
                    slices.push(project::Slice::new(tp));
                }
                MidiMessage::NoteOff { key: _, vel: _ }
                | MidiMessage::Aftertouch { key: _, vel: _ }
                | MidiMessage::Controller {
                    controller: _,
                    value: _,
                }
                | MidiMessage::ProgramChange { program: _ }
                | MidiMessage::ChannelAftertouch { vel: _ }
                | MidiMessage::PitchBend { bend: _ } => (),
            },
        }
    }

    Ok(slices)
}
