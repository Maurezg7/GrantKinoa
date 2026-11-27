use crate::engine::{Cmd, Engine, PlayMode, SampleData, Shared};
use crate::project::Project;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Renders the song (or one pass of `pattern` when the playlist is empty or `mode` is Pattern).
pub fn render(
    project: &Project,
    samples: &HashMap<u32, Arc<SampleData>>,
    sr: u32,
    mode: PlayMode,
    pattern: usize,
) -> Vec<f32> {
    let song = mode == PlayMode::Song && project.song_length() > 0;
    let steps = if song {
        project.song_length()
    } else {
        project
            .patterns
            .get(pattern)
            .map(|p| p.length)
            .unwrap_or(16)
    };
    let mut engine = Engine::new(
        sr as f32,
        Arc::new(project.clone()),
        Arc::new(Shared::default()),
    );
    for (ch, data) in samples {
        engine.handle(Cmd::Sample(*ch, Some(data.clone())));
    }
    engine.handle(Cmd::Mode(if song {
        PlayMode::Song
    } else {
        PlayMode::Pattern
    }));
    engine.handle(Cmd::CurrentPattern(pattern));
    engine.set_looping(false);
    engine.handle(Cmd::Play);

    let sps = project.bpm as f64 / 60.0 * 4.0 / sr as f64;
    let frames = (steps as f64 / sps).ceil() as usize;
    let tail = sr as usize * 3;
    let mut out = vec![0.0f32; (frames + tail) * 2];
    engine.render(&mut out);

    let threshold = 1e-4;
    let last = out.iter().rposition(|x| x.abs() > threshold).unwrap_or(0) / 2 + 1;
    out.truncate(last.max(frames).min(frames + tail) * 2);
    out
}

pub fn write_wav(path: &Path, data: &[f32], sr: u32) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in data {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    w.finalize()
}

pub fn load_wav(path: &Path) -> Result<SampleData, hound::Error> {
    let mut r = hound::WavReader::open(path)?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let frames = raw
        .chunks(ch)
        .map(|f| if ch == 1 { [f[0], f[0]] } else { [f[0], f[1]] })
        .collect();
    Ok(SampleData {
        sr: spec.sample_rate as f32,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip() {
        let p = Project::demo();
        let data = render(&p, &HashMap::new(), 22050, PlayMode::Pattern, 0);
        let path = std::env::temp_dir().join("grantkinoa-test.wav");
        write_wav(&path, &data, 22050).unwrap();
        let back = load_wav(&path).unwrap();
        assert_eq!(back.frames.len(), data.len() / 2);
        assert_eq!(back.sr, 22050.0);
        let _ = std::fs::remove_file(path);
    }
}
