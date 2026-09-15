use crate::project::Slice;

use super::*;

#[test]
fn num_samples() -> Result<(), crate::Error> {
    let num_samples = calculate_num_samples(
        TimePoint::default(),
        TimePoint::from(5.0),
        crate::project::SampleRate(48000),
        1,
        &[BPMChange {
            time_point: TimePoint {
                measure: 0,
                submeasure: 0.0,
            },
            bpm: 180.0,
        }],
    )?;

    assert_eq!(num_samples, 320_000, "bad num_samples");

    Ok(())
}

#[test]
fn half_bpm_120_to_60() -> Result<(), crate::Error> {
    const NUM_CHANNELS: u16 = 2;

    let x = calculate_num_samples(
        TimePoint::from(3.5),
        TimePoint::from(5.5),
        crate::project::SampleRate(44100),
        NUM_CHANNELS,
        &[
            BPMChange {
                time_point: TimePoint {
                    measure: 0,
                    submeasure: 0.0,
                },
                bpm: 120.0,
            },
            BPMChange {
                time_point: TimePoint {
                    measure: 4,
                    submeasure: 0.0,
                },
                bpm: 60.0,
            },
        ],
    )?;

    let expected = 2.0 // num channels
        * 44100.0 // sample rate
        * ((60.0 / 120.0) * 2.0 + (60.0 / 60.0) * 1.5 * BEATS_PER_MEASURE as f64);
    assert_eq!(x, expected as usize);

    Ok(())
}

#[test]
fn cuts() -> Result<(), crate::Error> {
    let (starting_sample, sample_counts) = slices_to_sample_counts(
        48000.into(),
        1,
        &TEST_TIME_POINTS,
        &[BPMChange {
            time_point: Default::default(),
            bpm: 180.0,
        }],
    )?;

    // First slice is not at zero
    assert_ne!(starting_sample, 0);
    // Check the actual value
    assert_eq!(starting_sample, 320_000);

    // There should be 38 of them
    assert_eq!(sample_counts.len(), TEST_TIME_POINTS.len());

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

const TEST_TIME_POINTS: [crate::project::Slice; 38] = [
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.0,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.125,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.25,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.375,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.5,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.65625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.6875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.75,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 5,
            submeasure: 0.875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.0,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.125,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.25,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.375,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.5,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.75,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 6,
            submeasure: 0.875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.0,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.125,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.25,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.375,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.5,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.75,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 7,
            submeasure: 0.875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.0,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.125,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.25,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.375,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.5,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.5625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.625,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.6875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.75,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.8125,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.875,
        },
    },
    Slice {
        time_point: TimePoint {
            measure: 8,
            submeasure: 0.9375,
        },
    },
];
