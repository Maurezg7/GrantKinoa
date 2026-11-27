use crate::engine::{Cmd, Engine, Shared};
use crate::project::Project;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Audio {
    tx: Sender<Cmd>,
    pub shared: Arc<Shared>,
    pub status: String,
    _stream: Option<cpal::Stream>,
}

type BoxError = Box<dyn std::error::Error>;

impl Audio {
    pub fn start(project: Arc<Project>) -> Self {
        let shared = Arc::new(Shared::default());
        let (tx, rx) = crossbeam_channel::unbounded();
        match open_stream(rx.clone(), shared.clone(), project.clone()) {
            Ok((stream, _sr, status)) => Self {
                tx,
                shared,
                status,
                _stream: Some(stream),
            },
            Err(e) => {
                let sr = 44100;
                spawn_null_output(rx, shared.clone(), project, sr);
                Self {
                    tx,
                    shared,
                    status: format!("Sin dispositivo de audio ({e})"),
                    _stream: None,
                }
            }
        }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

fn open_stream(
    rx: Receiver<Cmd>,
    shared: Arc<Shared>,
    project: Arc<Project>,
) -> Result<(cpal::Stream, u32, String), BoxError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no hay salida por defecto")?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let sr = config.sample_rate;
    let engine = Engine::new(sr as f32, project, shared);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, engine, rx)?,
        SampleFormat::F64 => build::<f64>(&device, config, engine, rx)?,
        SampleFormat::I16 => build::<i16>(&device, config, engine, rx)?,
        SampleFormat::I32 => build::<i32>(&device, config, engine, rx)?,
        SampleFormat::U16 => build::<u16>(&device, config, engine, rx)?,
        other => return Err(format!("formato no soportado: {other}").into()),
    };
    stream.play()?;
    Ok((stream, sr, format!("{} · {sr} Hz", host.id().name())))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut engine: Engine,
    rx: Receiver<Cmd>,
) -> Result<cpal::Stream, BoxError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch = vec![0.0f32; 8192];
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            while let Ok(cmd) = rx.try_recv() {
                engine.handle(cmd);
            }
            let frames = data.len() / channels.max(1);
            if scratch.len() < frames * 2 {
                scratch.resize(frames * 2, 0.0);
            }
            let buf = &mut scratch[..frames * 2];
            engine.render(buf);
            for (frame, s) in data.chunks_mut(channels).zip(buf.chunks(2)) {
                if channels == 1 {
                    frame[0] = T::from_sample((s[0] + s[1]) * 0.5);
                } else {
                    frame[0] = T::from_sample(s[0]);
                    frame[1] = T::from_sample(s[1]);
                    for x in &mut frame[2..] {
                        *x = T::from_sample(0.0);
                    }
                }
            }
        },
        |err| eprintln!("GrantKinoa audio: {err}"),
        None,
    )?;
    Ok(stream)
}

/// Keeps the transport running in real time when no audio device is available.
fn spawn_null_output(rx: Receiver<Cmd>, shared: Arc<Shared>, project: Arc<Project>, sr: u32) {
    std::thread::spawn(move || {
        let mut engine = Engine::new(sr as f32, project, shared);
        let frames = sr as usize / 100;
        let mut buf = vec![0.0f32; frames * 2];
        let mut next = Instant::now();
        loop {
            loop {
                match rx.try_recv() {
                    Ok(cmd) => engine.handle(cmd),
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                }
            }
            engine.render(&mut buf);
            next += Duration::from_millis(10);
            if let Some(d) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            } else {
                next = Instant::now();
            }
        }
    });
}
