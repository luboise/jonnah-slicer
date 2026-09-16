use super::*;

#[test]
fn mono_sample_index_test() -> Result<(), crate::Error> {
    let timing = Timing {
        bpm_changes: vec![BPMChange {
            time_point: TimePoint::from_submeasure(0, 0, 1),
            bpm: 160.0,
        }],
    };

    let tp = TimePoint::ZERO;
    assert_eq!(0, tp.samples_from_start(44100, &timing)?);

    let tp = TimePoint::from_submeasure(1, 0, 1);
    assert_eq!(66_150, tp.samples_from_start(44100, &timing)?);

    Ok(())
}

#[test]
fn ratio_test() {
    let start = TimePoint::new(7, 10);
    let end = start + TimePoint::from_submeasure(4, 0, 1);

    let res = start.ratio(&end, TimePoint::new(1, 2));
    assert_eq!(res, TimePoint::new(27, 10));
}

#[test]
fn add_test() {
    let lhs = TimePoint::from_integer(8);
    let rhs = -TimePoint::from_submeasure(2, 3, 10);

    // 5.7
    assert_eq!(lhs + rhs, TimePoint::new(57, 10));
}

#[test]
fn sub_test() {
    let lhs = TimePoint::from_integer(8);
    let rhs = TimePoint::from_submeasure(2, 3, 10);

    // 5.7
    assert_eq!(lhs - rhs, TimePoint::new(57, 10));
}

#[test]
fn sub_test_2() {
    let right = TimePoint::from_submeasure(5, 1, 2);
    let left = TimePoint::from_submeasure(2, 6, 10);

    assert_eq!(right - left, TimePoint::from_submeasure(2, 9, 10));
}

#[test]
fn neg_test() {
    let tp = TimePoint::from_submeasure(2, 3, 10);
    assert_eq!(-tp, TimePoint::from_submeasure(-2, -3, 10));
}

#[test]
fn quantise_1_2() {
    assert_eq!(
        TimePoint::from_submeasure(0, 6, 10).quantised(Snapping::Measure(2)),
        TimePoint::from_submeasure(0, 5, 10),
    );
}

#[test]
fn quantise_1_3() {
    assert_eq!(
        TimePoint::from_integer(1),
        TimePoint::from_submeasure(1, 3, 100).quantised(Snapping::Measure(3)),
    );

    assert_eq!(
        TimePoint::new(4, 3),
        TimePoint::from_submeasure(1, 3, 10).quantised(Snapping::Measure(3)),
    );

    assert_eq!(
        TimePoint::new(4, 3),
        TimePoint::from_submeasure(1, 4, 10).quantised(Snapping::Measure(3)),
    );
}

#[test]
fn quantise_1_4() {
    assert_eq!(
        TimePoint::from_integer(1),
        TimePoint::from_submeasure(1, 1, 10).quantised(Snapping::Measure(4)),
    );
    assert_eq!(
        TimePoint::from_integer(1),
        TimePoint::from_submeasure(1, 1, 10).quantised(Snapping::Beat(1)),
    );

    assert_eq!(
        TimePoint::new(5, 4),
        TimePoint::from_submeasure(1, 15, 100).quantised(Snapping::Measure(4)),
    );
    assert_eq!(
        TimePoint::new(5, 4),
        TimePoint::from_submeasure(1, 15, 100).quantised(Snapping::Beat(1)),
    );
}

#[test]
fn get_timepoint() {
    let bpm_changes = Timing {
        bpm_changes: [
            BPMChange {
                time_point: TimePoint::ZERO,
                bpm: 120.0,
            },
            BPMChange {
                time_point: TimePoint::from_measure(1),
                bpm: 60.0,
            },
        ]
        .into(),
    };

    // 2 seconds for first measure (120 bpm * 4 beats = 60 / 120 * 4 = 2)
    // 4 seconds for measures thereafter (60 bpm * 4 beats = 60 / 60 * 4 = 4)
    // 10 seconds = 2 seconds (120bpm) + 2 * 4 seconds (60bpm)
    let time_points = bpm_changes.get_timepoints(&[10 * 1_000_000, 10 * 1_000_000 + 500_000]);
    assert_eq!(time_points.len(), 2);

    assert_eq!(time_points[1], TimePoint::from_submeasure(3, 1, 2));

    assert_eq!(
        time_points[0].quantised(Snapping::Measure(1)),
        TimePoint::from_integer(3)
    );
}

fn loves_me_not_bpm_changes() -> [BPMChange; 15] {
    [
        BPMChange {
            time_point: TimePoint::ZERO,
            bpm: 210.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(20),
            bpm: 205.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(20, 1, 2),
            bpm: 200.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(21),
            bpm: 195.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(21, 1, 2),
            bpm: 190.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(22),
            bpm: 185.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(22, 1, 2),
            bpm: 180.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(23),
            bpm: 175.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(24),
            bpm: 180.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(47, 1, 4),
            bpm: 185.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(47, 1, 2),
            bpm: 190.0,
        },
        BPMChange {
            time_point: TimePoint::from_submeasure(47, 3, 4),
            bpm: 195.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(48),
            bpm: 200.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(54),
            bpm: 205.0,
        },
        BPMChange {
            time_point: TimePoint::from_measure(56),
            bpm: 210.0,
        },
    ]
}
