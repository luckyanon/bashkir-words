// Rebuild the CC0 score and sound effects for the native Android audio player.
// rustc -O rust/app/examples/render_music.rs -o render_music
// ./render_music public/assets
use std::env;
use std::f64::consts::TAU;
use std::fs::{File, create_dir_all};
use std::io::{self, BufWriter, Write};
use std::path::Path;

const SAMPLE_RATE: u32 = 22_050;
const BEAT: f64 = 60.0 / 66.0;
const PARTIALS: [f64; 6] = [1.0, 0.32, 0.12, 0.045, 0.025, 0.012];
const SCORE: [(u8, f64, u8); 35] = [
    (74, 2.5, 0),
    (76, 0.5, 0),
    (78, 1.0, 0),
    (81, 2.5, 78),
    (78, 0.5, 0),
    (76, 1.0, 0),
    (74, 3.0, 0),
    (0, 1.5, 0),
    (71, 1.0, 0),
    (74, 2.0, 71),
    (76, 1.0, 0),
    (78, 2.0, 0),
    (76, 1.0, 0),
    (74, 1.0, 0),
    (71, 2.0, 0),
    (69, 2.5, 0),
    (0, 1.5, 0),
    (74, 1.5, 0),
    (78, 0.5, 0),
    (81, 2.0, 0),
    (83, 1.0, 0),
    (86, 3.0, 83),
    (83, 1.0, 0),
    (81, 2.0, 0),
    (78, 2.5, 0),
    (0, 1.5, 0),
    (81, 1.0, 0),
    (78, 1.0, 0),
    (76, 2.0, 74),
    (74, 1.0, 0),
    (71, 1.0, 0),
    (69, 2.0, 0),
    (71, 1.0, 0),
    (74, 4.0, 0),
    (0, 3.0, 0),
];

fn samples(time: f64) -> usize {
    (time * f64::from(SAMPLE_RATE)).round() as usize
}

fn frequency(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

fn ramp(t: f64, start: f64, end: f64) -> f64 {
    ((t - start) / (end - start)).clamp(0.0, 1.0)
}

fn waveform(phase: f64) -> f64 {
    PARTIALS
        .iter()
        .enumerate()
        .map(|(i, gain)| gain * (phase * (i + 1) as f64).sin())
        .sum()
}

fn generate_music() -> Vec<f64> {
    let mut time = 0.0;
    let mut audio = vec![0.0; samples(SCORE.iter().map(|(_, beats, _)| beats * BEAT).sum())];
    let mut seed = 9173_u32;
    let air: Vec<_> = (0..samples(2.0))
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            f64::from(seed) / 2147483648.0 - 1.0
        })
        .collect();
    let normalizer = (0..4096)
        .map(|index| waveform(f64::from(index) * TAU / 4096.0).abs())
        .fold(1.0_f64, f64::max);
    for (note, beats, grace) in SCORE {
        let duration = beats * BEAT * 0.93;
        let offset = samples(time);
        time += beats * BEAT;
        if note == 0 {
            continue;
        }
        let attack = (duration * 0.22).min(0.1);
        let release = duration - (duration * 0.3).min(0.23);
        let vibrato_delay = (duration * 0.35).min(0.3);
        let vibrato_depth = if duration > 0.8 { 7.0 } else { 2.0 };
        let main_frequency = frequency(note);
        let ornament = if grace == 0 {
            main_frequency
        } else {
            frequency(grace)
        };
        let omega = TAU * (1400.0 + main_frequency * 0.35) / f64::from(SAMPLE_RATE);
        let alpha = omega.sin() / (2.0 * 0.65);
        let b0 = alpha / (1.0 + alpha);
        let a1 = -2.0 * omega.cos() / (1.0 + alpha);
        let a2 = (1.0 - alpha) / (1.0 + alpha);
        let mut previous_air = [0.0; 2];
        let mut previous_filter = [0.0; 2];
        let mut phase = 0.0;
        for i in 0..samples(duration) {
            let t = i as f64 / f64::from(SAMPLE_RATE);
            let bend = -11.0 + 12.5 * ramp(t, 0.0, attack) - 6.5 * ramp(t, attack, duration);
            let vibrato = vibrato_depth
                * ramp(t, vibrato_delay, duration * 0.65)
                * (TAU * (4.7 + f64::from(note % 3) * 0.13) * t).sin();
            let note_frequency = if t < 0.075 { ornament } else { main_frequency };
            let hz = note_frequency * 2.0_f64.powf((bend + vibrato) / 1200.0);
            phase += TAU * hz / f64::from(SAMPLE_RATE);
            let air_sample = air[i % air.len()];
            let filtered = b0 * (air_sample - previous_air[1])
                - a1 * previous_filter[0]
                - a2 * previous_filter[1];
            previous_air = [air_sample, previous_air[0]];
            previous_filter = [filtered, previous_filter[0]];
            let envelope = if t < release {
                0.19 * ramp(t, 0.0, attack) - 0.04 * ramp(t, attack, duration * 0.55)
            } else {
                0.14 * (1.0 - ramp(t, release, duration))
            };
            audio[offset + i] = (waveform(phase) / normalizer + filtered * 0.075) * envelope * 0.25;
        }
    }
    audio
}

fn generate_effect(name: &str, frequencies: &[f64]) -> Vec<f64> {
    let spacing = if name == "repeat" { 0.14 } else { 0.08 };
    let mut audio = vec![0.0; samples((frequencies.len() - 1) as f64 * spacing) + samples(0.12)];
    let peak: f64 = match name {
        "invalid" => 0.025,
        "repeat" => 0.075,
        _ => 0.055,
    };
    for (note, &hz) in frequencies.iter().enumerate() {
        let offset = samples(note as f64 * spacing);
        let mut phase = 0.0;
        for i in 0..samples(0.12) {
            let t = i as f64 / f64::from(SAMPLE_RATE);
            let frequency = if name == "invalid" {
                hz * (155.0 / hz).powf(ramp(t, 0.0, 0.1))
            } else {
                hz
            };
            phase += TAU * frequency / f64::from(SAMPLE_RATE);
            let amplitude = if t < 0.01 {
                0.0001 * (peak / 0.0001).powf(ramp(t, 0.0, 0.01))
            } else {
                peak * (0.0001 / peak).powf(ramp(t, 0.01, 0.11))
            };
            audio[offset + i] += phase.sin() * amplitude;
        }
    }
    audio
}

fn write_wave(path: &Path, audio: &[f64]) -> io::Result<()> {
    let mut stream = BufWriter::new(File::create(path)?);
    let size = (audio.len() * 2) as u32;
    stream.write_all(b"RIFF")?;
    stream.write_all(&(36 + size).to_le_bytes())?;
    stream.write_all(b"WAVEfmt ")?;
    stream.write_all(&16_u32.to_le_bytes())?;
    stream.write_all(&[1, 0, 1, 0])?;
    stream.write_all(&SAMPLE_RATE.to_le_bytes())?;
    stream.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?;
    stream.write_all(&[2, 0, 16, 0])?;
    stream.write_all(b"data")?;
    stream.write_all(&size.to_le_bytes())?;
    for &sample in audio {
        let pcm = (sample.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16;
        stream.write_all(&pcm.to_le_bytes())?;
    }
    stream.flush()
}

fn main() -> io::Result<()> {
    let directory = env::args().nth(1).unwrap_or_else(|| "public/assets".into());
    let directory = Path::new(&directory);
    create_dir_all(directory)?;
    write_wave(&directory.join("native-music.wav"), &generate_music())?;
    for (name, notes) in [
        ("select", &[392.0][..]),
        ("target", &[523.0, 659.0]),
        ("bonus", &[659.0, 784.0]),
        ("repeat", &[784.0, 659.0]),
        ("invalid", &[196.0]),
        ("hint", &[440.0, 523.0]),
        ("victory", &[523.0, 659.0, 784.0, 1047.0]),
    ] {
        write_wave(
            &directory.join(format!("native-sound-{name}.wav")),
            &generate_effect(name, notes),
        )?;
    }
    Ok(())
}
