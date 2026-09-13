use super::*;

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
