use super::*;

const EPSILON: f64 = 0.001;

fn assert_close(a: f64, b: f64) {
    assert!((b - a).abs() <= EPSILON);
}

#[test]
fn mono_sample_index_test() -> Result<(), crate::Error> {
    let bpm_changes = vec![BPMChange {
        time_point: TimePoint::new(0, 0.0),
        bpm: 160.0,
    }];

    let tp = TimePoint::new(0, 0.0);
    assert_eq!(0, tp.samples_from_start(44100, &bpm_changes)?);

    let tp = TimePoint::new(1, 0.0);
    assert_eq!(66_150, tp.samples_from_start(44100, &bpm_changes)?);

    Ok(())
}

#[test]
fn ratio_test() {
    let start = TimePoint::new(0, 0.7);
    let end = start + TimePoint::new(4, 0.0);

    let res = start.ratio(&end, 0.5);
    assert_eq!(f64::from(res), 2.7);
}

#[test]
fn add_test() {
    let lhs = TimePoint::new(8, 0.0);
    let rhs = -TimePoint::new(2, 0.3);

    assert_eq!(f64::from(lhs + rhs), 5.7);
}

#[test]
fn sub_test() {
    let lhs = TimePoint::new(8, 0.0);
    let rhs = TimePoint::new(2, 0.3);

    assert_eq!(f64::from(lhs - rhs), 5.7);
}

#[test]
fn sub_test_2() {
    let right = TimePoint::new(5, 0.5);
    let left = TimePoint::new(2, 0.6);

    assert_eq!(
        right - left,
        TimePoint {
            measure: 2,
            submeasure: 0.9
        }
    );
}

#[test]
fn neg_test() {
    let tp = TimePoint {
        measure: 2,
        submeasure: 0.3,
    };

    assert_close(f64::from(-tp), -2.3);
}

#[test]
fn quantise_1_2() {
    assert_eq!(
        f64::from(TimePoint::new(0, 0.6).quantised(Snapping::Measure(2))),
        f64::from(TimePoint {
            measure: 0,
            submeasure: 0.5,
        }),
    );
}

#[test]
fn quantise_1_3() {
    assert_close(
        1.0,
        f64::from(TimePoint::new(1, 0.03).quantised(Snapping::Measure(3))),
    );

    assert_close(
        1.33333,
        f64::from(TimePoint::new(1, 0.3).quantised(Snapping::Measure(3))),
    );

    assert_close(
        1.33333,
        f64::from(TimePoint::new(1, 0.4).quantised(Snapping::Measure(3))),
    );
}

#[test]
fn quantise_1_4() {
    assert_close(
        1.0,
        f64::from(TimePoint::new(1, 0.1).quantised(Snapping::Measure(4))),
    );
    assert_close(
        1.0,
        f64::from(TimePoint::new(1, 0.1).quantised(Snapping::Beat(1))),
    );

    assert_close(
        1.25,
        f64::from(TimePoint::new(1, 0.15).quantised(Snapping::Measure(4))),
    );
    assert_close(
        1.25,
        f64::from(TimePoint::new(1, 0.15).quantised(Snapping::Beat(1))),
    );
}

#[test]
fn from_time() -> Result<(), crate::Error> {
    let bpm_changes = [
        BPMChange {
            time_point: TimePoint {
                measure: 0,
                submeasure: 0.0,
            },
            bpm: 120.0,
        },
        BPMChange {
            time_point: TimePoint {
                measure: 1,
                submeasure: 0.0,
            },
            bpm: 60.0,
        },
    ];

    // 2 seconds for first measure (120 bpm * 4 beats = 60 / 120 * 4 = 2)
    // 4 seconds for measures thereafter (60 bpm * 4 beats = 60 / 60 * 4 = 4)
    // 10 seconds = 2 seconds (120bpm) + 2 * 4 seconds (60bpm)
    let time_point = TimePoint::from_time(10.0, &bpm_changes)?;

    assert_eq!(f64::from(time_point.quantised(Snapping::Measure(1))), 3.0);

    Ok(())
}

const LOVES_ME_NOT_BPM_CHANGES: [BPMChange; 15] = [
    BPMChange {
        time_point: TimePoint {
            measure: 0,
            submeasure: 0.0,
        },
        bpm: 210.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 20,
            submeasure: 0.0,
        },
        bpm: 205.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 20,
            submeasure: 0.5,
        },
        bpm: 200.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 21,
            submeasure: 0.0,
        },
        bpm: 195.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 21,
            submeasure: 0.5,
        },
        bpm: 190.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 22,
            submeasure: 0.0,
        },
        bpm: 185.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 22,
            submeasure: 0.5,
        },
        bpm: 180.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 23,
            submeasure: 0.0,
        },
        bpm: 175.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 24,
            submeasure: 0.0,
        },
        bpm: 180.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 47,
            submeasure: 0.25,
        },
        bpm: 185.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 47,
            submeasure: 0.50,
        },
        bpm: 190.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 47,
            submeasure: 0.75,
        },
        bpm: 195.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 48,
            submeasure: 0.0,
        },
        bpm: 200.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 54,
            submeasure: 0.0,
        },
        bpm: 205.0,
    },
    BPMChange {
        time_point: TimePoint {
            measure: 56,
            submeasure: 0.0,
        },
        bpm: 210.0,
    },
];
