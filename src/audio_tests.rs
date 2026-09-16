use crate::project::Slice;

use super::*;

#[test]
fn num_samples() -> Result<(), crate::Error> {
    let num_samples = calculate_num_samples(
        TimePoint::ZERO,
        TimePoint::from_integer(5),
        crate::project::SampleRate(48000),
        1,
        &Timing {
            bpm_changes: [BPMChange {
                time_point: TimePoint::ZERO,
                bpm: 180.0,
            }]
            .into(),
        },
    )?;

    assert!(
        (319_999usize..=320_001).contains(&num_samples),
        "bad num_samples: {num_samples}"
    );

    Ok(())
}

#[test]
fn half_bpm_120_to_60() -> Result<(), crate::Error> {
    const NUM_CHANNELS: u16 = 2;

    let num_samples = calculate_num_samples(
        TimePoint::from_submeasure(3, 1, 2),
        TimePoint::from_submeasure(5, 1, 2),
        crate::project::SampleRate(44100),
        NUM_CHANNELS,
        &Timing {
            bpm_changes: [
                BPMChange {
                    time_point: TimePoint::ZERO,
                    bpm: 120.0,
                },
                BPMChange {
                    time_point: TimePoint::from_integer(4),
                    bpm: 60.0,
                },
            ]
            .into(),
        },
    )?;

    let expected = 2.0 // num channels
        * 44100.0 // sample rate
        * ((60.0 / 120.0) * 2.0 + (60.0 / 60.0) * 1.5 * BEATS_PER_MEASURE as f64);
    assert_eq!(num_samples, expected as usize);

    Ok(())
}

// TODO: Fix this test, all of the samples are += 1
/*
#[test]
fn cuts() -> Result<(), crate::Error> {
    const BEATS_PER_SECOND: usize = 3;
    const NUM_SAMPLES: usize = 16 * 48000 / BEATS_PER_SECOND;

    let (starting_sample, sample_counts) = slices_to_sample_counts(
        48000.into(),
        1,
        &test_time_points(),
        &Timing {
            bpm_changes: [BPMChange {
                time_point: Default::default(),
                bpm: 180.0,
            }]
            .into(),
        },
        NUM_SAMPLES,
    )?;

    // First slice is not at zero
    assert_ne!(starting_sample, 0);
    // // Check the actual value
    // assert_eq!(starting_sample, 320_000);

    // There should be 38 of them
    assert_eq!(sample_counts.len(), test_time_points().len());

    assert_eq!(
        sample_counts[0..38],
        [
            4, 4, 4, 4, // beat 1
            4, 1, 1, 2, 4, 4, // beat 3
            4, 4, 4, 4, // beat 5
            4, 4, 4, 4, // beat 7
            4, 4, 4, 4, // beat 9
            4, 4, 4, 4, // beat 11
            4, 4, 4, 4, // beat 13
            2, 2, 2, 2, 2, 2, 2, 2 // beat 15-16
        ]
        // Length of a half beat is 2000 samples at 180 BPM
        .map(|v| v * 2000),
    );

    Ok(())
}
*/

fn test_time_points() -> [Slice; 38] {
    [
        TimePoint::from_submeasure(5, 0, 8),
        TimePoint::from_submeasure(5, 1, 8),
        TimePoint::from_submeasure(5, 2, 8),
        TimePoint::from_submeasure(5, 3, 8),
        // Beat 3
        TimePoint::from_submeasure(5, 16, 32),
        TimePoint::from_submeasure(5, 20, 32),
        TimePoint::from_submeasure(5, 21, 32),
        TimePoint::from_submeasure(5, 22, 32),
        TimePoint::from_submeasure(5, 24, 32),
        TimePoint::from_submeasure(5, 28, 32),
        // Beat 5
        TimePoint::from_submeasure(6, 0, 8),
        TimePoint::from_submeasure(6, 1, 8),
        TimePoint::from_submeasure(6, 2, 8),
        TimePoint::from_submeasure(6, 3, 8),
        TimePoint::from_submeasure(6, 4, 8),
        TimePoint::from_submeasure(6, 5, 8),
        TimePoint::from_submeasure(6, 6, 8),
        TimePoint::from_submeasure(6, 7, 8),
        // Beat 9
        TimePoint::from_submeasure(7, 0, 8),
        TimePoint::from_submeasure(7, 1, 8),
        TimePoint::from_submeasure(7, 2, 8),
        TimePoint::from_submeasure(7, 3, 8),
        TimePoint::from_submeasure(7, 4, 8),
        TimePoint::from_submeasure(7, 5, 8),
        TimePoint::from_submeasure(7, 6, 8),
        TimePoint::from_submeasure(7, 7, 8),
        // Beat 9
        TimePoint::from_submeasure(8, 0, 8),
        TimePoint::from_submeasure(8, 1, 8),
        TimePoint::from_submeasure(8, 2, 8),
        TimePoint::from_submeasure(8, 3, 8),
        TimePoint::from_submeasure(8, 8, 16),
        TimePoint::from_submeasure(8, 9, 16),
        TimePoint::from_submeasure(8, 10, 16),
        TimePoint::from_submeasure(8, 11, 16),
        TimePoint::from_submeasure(8, 12, 16),
        TimePoint::from_submeasure(8, 13, 16),
        TimePoint::from_submeasure(8, 14, 16),
        TimePoint::from_submeasure(8, 15, 16),
    ]
    .map(|time_point| -> Slice { Slice { time_point } })
}
