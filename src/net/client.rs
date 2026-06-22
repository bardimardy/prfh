use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use anyhow::{anyhow, Result};

use crate::game::world::WorldView;
use crate::net::protocol::{decode_line, encode_line, ClientMsg, InputEvent, ServerMsg};

pub struct ClientHandle {
    write: TcpStream,
    pub rx: Receiver<ServerMsg>,
}

impl ClientHandle {
    pub fn send_input(&mut self, ev: InputEvent) {
        let _ = self
            .write
            .write_all(encode_line(&ClientMsg::Input(ev)).as_bytes());
    }
}

/// Connect, perform the Hello/Welcome handshake, and start the reader thread.
pub fn connect(addr: &str, name: &str) -> Result<(WorldView, ClientHandle)> {
    let stream = TcpStream::connect(addr)?;
    let mut write = stream.try_clone()?;
    write.write_all(
        encode_line(&ClientMsg::Hello {
            name: name.to_string(),
        })
        .as_bytes(),
    )?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(anyhow!("Verbindung vom Host geschlossen"));
    }
    let mut world = match decode_line::<ServerMsg>(&line)? {
        ServerMsg::Welcome {
            your_id,
            color,
            players,
        } => {
            let mut w = WorldView::new(your_id);
            w.apply(ServerMsg::Welcome {
                your_id,
                color,
                players,
            });
            w
        }
        ServerMsg::Reject { reason } => return Err(anyhow!(reason)),
        _ => return Err(anyhow!("unerwartete erste Nachricht vom Host")),
    };
    let _ = &mut world;

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => match decode_line::<ServerMsg>(&line) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Err(_) => break,
            }
        }
    });

    Ok((world, ClientHandle { write, rx }))
}
