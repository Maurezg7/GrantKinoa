#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod dsp;
mod engine;
mod export;
mod midi;
mod plugins;
mod project;
mod templates;
mod ui;

use std::collections::HashMap;
use std::path::PathBuf;

fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 34.0);
            let d = (dx * dx + dy * dy).sqrt();
            let leaf = ((x as f32 - 40.0).powi(2) / 60.0 + (y as f32 - 8.0).powi(2) / 14.0) < 1.0;
            let px = if leaf {
                [110, 196, 98, 255]
            } else if d < 26.0 && d > 15.0 {
                [255, 157, 46, 255]
            } else if d <= 15.0 && d > 9.0 && !(dx > 0.0 && dy < 0.0) {
                [255, 200, 120, 255]
            } else if d <= 28.0 {
                [44, 49, 53, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[(y * n + x) * 4..][..4].copy_from_slice(&px);
        }
    }
    egui::IconData {
        rgba,
        width: n as u32,
        height: n as u32,
    }
}

/// `grantkinoa --render <proyecto.gkp|--demo> <salida.wav>` renders without opening the window.
fn render_cli(args: &[String]) -> Result<(), String> {
    let [input, output] = args else {
        return Err("uso: grantkinoa --render <proyecto.gkp|--demo> <salida.wav>".into());
    };
    let mut project = if input == "--demo" {
        project::Project::demo()
    } else {
        let s = std::fs::read_to_string(input).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())?
    };
    project.sanitize();
    let mut samples = HashMap::new();
    for ch in &project.channels {
        if let Some(p) = &ch.instrument.sampler.path
            && ch.instrument.kind == project::InstrumentKind::Sampler
        {
            let d = export::load_wav(p.as_ref()).map_err(|e| format!("{p}: {e}"))?;
            samples.insert(ch.id, std::sync::Arc::new(d));
        }
    }
    let data = export::render(&project, &samples, 44100, engine::PlayMode::Song, 0);
    export::write_wav(output.as_ref(), &data, 44100).map_err(|e| e.to_string())?;
    println!("{output}: {:.1} s", data.len() as f32 / 2.0 / 44100.0);
    Ok(())
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--render") {
        if let Err(e) = render_cli(&args[1..]) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let open = args.first().map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GrantKinoa")
            .with_app_id("grantkinoa")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([960.0, 600.0])
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native(
        "GrantKinoa",
        options,
        Box::new(|cc| Ok(Box::new(ui::GrantKinoa::new(cc, open)))),
    )
}
