//! Entrada de teclados MIDI externos.

use crossbeam_channel::{Receiver, Sender, unbounded};
use midir::{Ignore, MidiInput, MidiInputConnection};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEvent {
    On(u8, f32),
    Off(u8),
}

pub struct Midi {
    conn: Option<MidiInputConnection<()>>,
    pub port: Option<String>,
    tx: Sender<MidiEvent>,
    pub rx: Receiver<MidiEvent>,
}

impl Midi {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            conn: None,
            port: None,
            tx,
            rx,
        }
    }

    pub fn ports() -> Vec<String> {
        MidiInput::new("GrantKinoa")
            .map(|m| {
                m.ports()
                    .iter()
                    .filter_map(|p| m.port_name(p).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn connect(&mut self, name: &str, ctx: egui::Context) -> Result<(), String> {
        self.disconnect();
        let mut m = MidiInput::new("GrantKinoa").map_err(|e| e.to_string())?;
        m.ignore(Ignore::All);
        let port = m
            .ports()
            .into_iter()
            .find(|p| m.port_name(p).ok().as_deref() == Some(name))
            .ok_or_else(|| "Puerto MIDI no encontrado".to_string())?;
        let tx = self.tx.clone();
        let conn = m
            .connect(
                &port,
                "grantkinoa-in",
                move |_, msg, _| {
                    if let Some(e) = parse(msg) {
                        let _ = tx.send(e);
                        ctx.request_repaint();
                    }
                },
                (),
            )
            .map_err(|e| e.to_string())?;
        self.conn = Some(conn);
        self.port = Some(name.into());
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(c) = self.conn.take() {
            c.close();
        }
        self.port = None;
    }
}

fn parse(msg: &[u8]) -> Option<MidiEvent> {
    match *msg {
        [s, k, v, ..] if s & 0xF0 == 0x90 && v > 0 => {
            Some(MidiEvent::On(k & 0x7F, v as f32 / 127.0))
        }
        [s, k, ..] if s & 0xF0 == 0x80 || s & 0xF0 == 0x90 => Some(MidiEvent::Off(k & 0x7F)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_messages() {
        assert_eq!(parse(&[0x90, 60, 127]), Some(MidiEvent::On(60, 1.0)));
        assert_eq!(parse(&[0x93, 64, 0]), Some(MidiEvent::Off(64)));
        assert_eq!(parse(&[0x80, 62, 40]), Some(MidiEvent::Off(62)));
        assert_eq!(parse(&[0xB0, 1, 64]), None);
    }
}
