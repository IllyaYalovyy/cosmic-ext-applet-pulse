use super::*;
use std::time::Duration;
fn sample(g: &mut Graphs, cpus: &[f32], v: Option<f32>, at: Instant, s: &Settings) {
    g.observe(cpus, [("ram".into(), vec![v])], at, s);
}
#[test]
fn one_saturated_core_is_preserved_and_history_is_bounded() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let s = Settings::default();
    let mut cpus = vec![0.0; 16];
    cpus[3] = 100.0;
    for i in 0..1000 {
        sample(&mut g, &cpus, Some(0.0), now + Duration::from_secs(i), &s);
    }
    assert_eq!(g.cpu.len(), 121);
    let p = g.cpu.back().unwrap();
    assert_eq!(p.average, 6.25);
    assert_eq!(p.busiest, 100.0);
    let (svg, _, _) = g.history_svg(&s, 16.0, false);
    assert!(svg.contains("#f36675"));
    assert!(svg.contains("stroke-dasharray"));
    assert!(svg.matches("<path").count() <= 7);
    assert!(svg.len() < 20_000);
}
#[test]
fn peaks_fade_by_elapsed_time_and_missing_values_clear_them() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let s = Settings::default();
    sample(&mut g, &[0.0], Some(100.0), now, &s);
    sample(&mut g, &[0.0], Some(20.0), now + Duration::from_secs(2), &s);
    assert_eq!(g.peaks["ram"][0].unwrap().value, 100.0);
    let (svg, _, _) = g.meter_svg("ram", &[Some(20.0)], &s, 16.0, false);
    assert!(svg.contains("opacity=\"0.600\""));
    sample(&mut g, &[0.0], None, now + Duration::from_secs(3), &s);
    assert!(g.peaks["ram"][0].is_none());
    sample(
        &mut g,
        &[0.0],
        Some(100.0),
        now + Duration::from_secs(4),
        &s,
    );
    sample(
        &mut g,
        &[0.0],
        Some(20.0),
        now + Duration::from_secs(13),
        &s,
    );
    assert_eq!(g.peaks["ram"][0].unwrap().value, 20.0);
}
#[test]
fn small_geometry_has_no_aspect_ratio_padding_and_vertical_fits() {
    let (_, gap, _, w, h) = geometry(16, 16.0, false, true);
    assert_eq!(gap, 1.0);
    assert_eq!(w, 63.0);
    assert_eq!(h, 16.0);
    for size in [12.0, 16.0, 24.0, 32.0] {
        let (_, _, _, w, _) = geometry(32, size, true, true);
        assert!(w <= size);
    }
}
#[test]
fn topology_change_and_sampler_failure_clear_old_history() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let s = Settings::default();
    sample(&mut g, &[100.0, 0.0], Some(100.0), now, &s);
    sample(&mut g, &[0.0], Some(10.0), now + Duration::from_secs(1), &s);
    assert_eq!(g.cpu.len(), 1);
    g.clear();
    assert!(g.cpu.is_empty());
    assert!(g.peaks.is_empty());
}
#[test]
fn unchanged_views_reuse_svg_handles() {
    let mut g = Graphs::default();
    let s = Settings::default();
    let now = Instant::now();
    sample(&mut g, &[0.0], Some(20.0), now, &s);
    let a = g.meter("ram", &[Some(20.0)], &s, 16.0, false);
    let b = g.meter("ram", &[Some(20.0)], &s, 16.0, false);
    assert_eq!(a.handle.id(), b.handle.id());
    sample(&mut g, &[0.0], Some(80.0), now + Duration::from_secs(2), &s);
    let c = g.meter("ram", &[Some(80.0)], &s, 16.0, false);
    assert_ne!(a.handle.id(), c.handle.id());
}

#[test]
fn disabled_glow_and_removed_devices_cannot_retain_peaks() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let mut s = Settings::default();
    sample(&mut g, &[100.0], Some(100.0), now, &s);
    s.peak_glow = false;
    sample(
        &mut g,
        &[10.0],
        Some(10.0),
        now + Duration::from_secs(2),
        &s,
    );
    assert!(g.peaks.is_empty());
    s.peak_glow = true;
    sample(
        &mut g,
        &[10.0],
        Some(100.0),
        now + Duration::from_secs(4),
        &s,
    );
    g.observe(&[0.0], [], now + Duration::from_secs(6), &s);
    assert!(g.peaks.is_empty());
}

#[test]
fn pause_does_not_connect_old_history_to_new_readings() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let s = Settings::default();
    sample(&mut g, &[100.0], Some(100.0), now, &s);
    sample(
        &mut g,
        &[0.0],
        Some(10.0),
        now + Duration::from_secs(60),
        &s,
    );
    assert_eq!(g.cpu.len(), 1);
    assert_eq!(g.peaks["ram"][0].unwrap().value, 10.0);
}

#[test]
#[ignore = "Manual timing of graph preparation; not a full app CPU benchmark"]
fn benchmark_graph_preparation() {
    for count in [16, 128] {
        let s = Settings::default();
        let mut g = Graphs::default();
        let now = Instant::now();
        let started = Instant::now();
        for frame in 0..1000 {
            let cpus: Vec<_> = (0..count)
                .map(|i| ((i * 17 + frame * 3) % 101) as f32)
                .collect();
            let core_values: Vec<_> = cpus.iter().copied().map(Some).collect();
            let meters: Vec<_> = (0..7)
                .map(|i| {
                    (
                        format!("metric:{i}"),
                        vec![Some(((frame + i * 7) % 101) as f32); 2],
                    )
                })
                .collect();
            g.observe(
                &cpus,
                meters.clone(),
                now + Duration::from_secs(frame as u64 * 2),
                &s,
            );
            std::hint::black_box(g.meter("cpu", &core_values, &s, 16.0, false));
            std::hint::black_box(g.history(&s, 16.0, false));
            for (key, values) in &meters {
                std::hint::black_box(g.meter(key, values, &s, 16.0, false));
            }
        }
        println!(
            "{count} cores + history + 7 paired meters: {:.1} µs per sample (1000 samples)",
            started.elapsed().as_secs_f64() * 1000.0
        );
        let started = Instant::now();
        for _ in 0..100_000 {
            std::hint::black_box(g.history(&s, 16.0, false));
        }
        println!(
            "Cached history handle lookup: {:.2} µs",
            started.elapsed().as_secs_f64() * 10.0
        );
    }
}

#[test]
#[ignore = "Export production SVG examples to PULSE_PREVIEW_DIR for visual inspection"]
fn export_graph_previews() {
    use std::fmt::Write;
    let directory = std::path::PathBuf::from(
        std::env::var("PULSE_PREVIEW_DIR").expect("Set PULSE_PREVIEW_DIR"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let now = Instant::now();
    let s = Settings::default();
    let mut g = Graphs::default();
    let mut cpus = vec![0.0; 16];
    for frame in 0..61 {
        cpus.fill(4.0 + (frame % 7) as f32);
        cpus[3] = if frame > 40 {
            100.0
        } else {
            20.0 + (frame % 13) as f32 * 5.0
        };
        g.observe(&cpus, [], now + Duration::from_secs(frame * 2), &s);
    }
    let cores = cpus.iter().copied().map(Some).collect::<Vec<_>>();
    let mut sheet = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="720" height="284"><rect width="720" height="284" rx="12" fill="#22252e"/><g font-family="sans-serif" fill="#e4e7ef"><text x="20" y="30" font-size="18">Pulse · small-panel graph preview</text><text x="20" y="54" font-size="12" fill="#aeb6c5">16 px graphs shown at 3× · one CPU core saturated</text>"##,
    );
    for (name, x, image) in [
        ("Cores", 20, g.meter_svg("cpu", &cores, &s, 16.0, false).0),
        (
            "Average + busiest core",
            310,
            g.history_svg(&s, 16.0, false).0,
        ),
    ] {
        write!(sheet,r#"<text x="{x}" y="86" font-size="13">{name}</text><g transform="translate({x},98) scale(3)">{image}</g>"#).unwrap();
    }
    sheet.push_str(r#"<text x="20" y="186" font-size="13">Peak afterglow after a drop from 100% to 20%</text>"#);
    for (index, age) in [0, 2, 6, 9].into_iter().enumerate() {
        let mut meter = Graphs::default();
        sample(&mut meter, &[0.0], Some(100.0), now, &s);
        // Feed every two seconds so the expiry case is not a sampling pause.
        for t in 1..=age.max(1) {
            sample(
                &mut meter,
                &[0.0],
                Some(20.0),
                now + Duration::from_secs(t),
                &s,
            );
        }
        let image = meter.meter_svg("ram", &[Some(20.0)], &s, 16.0, false).0;
        let x = 20 + index * 170;
        write!(sheet,r#"<g transform="translate({x},204) scale(3)">{image}</g><text x="{}" y="231" font-size="12">{} s</text>"#,x+30,age.max(1)).unwrap();
    }
    sheet.push_str("</g></svg>");
    std::fs::write(directory.join("graphs.svg"), sheet).unwrap();
    for size in [16.0, 24.0, 32.0] {
        std::fs::write(
            directory.join(format!("cores-{size}.svg")),
            g.meter_svg("cpu", &cores, &s, size, false).0,
        )
        .unwrap();
        std::fs::write(
            directory.join(format!("history-{size}.svg")),
            g.history_svg(&s, size, false).0,
        )
        .unwrap();
        std::fs::write(
            directory.join(format!("vertical-{size}.svg")),
            g.meter_svg("cpu", &cores, &s, size, true).0,
        )
        .unwrap();
    }
}

#[test]
fn slow_polling_still_shows_one_falling_sample_without_extra_ticks() {
    let mut g = Graphs::default();
    let now = Instant::now();
    let s = Settings {
        interval: 30,
        ..Settings::default()
    };
    sample(&mut g, &[0.0], Some(100.0), now, &s);
    sample(
        &mut g,
        &[0.0],
        Some(20.0),
        now + Duration::from_secs(30),
        &s,
    );
    assert_eq!(g.peaks["ram"][0].unwrap().value, 100.0);
    let (svg, _, _) = g.meter_svg("ram", &[Some(20.0)], &s, 16.0, false);
    assert!(svg.contains("opacity=\"0.400\""));
    sample(
        &mut g,
        &[0.0],
        Some(20.0),
        now + Duration::from_secs(60),
        &s,
    );
    assert_eq!(g.peaks["ram"][0].unwrap().value, 20.0);
}
