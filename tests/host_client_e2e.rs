use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use prfh::game::world::WorldView;
use prfh::net::client::connect;
use prfh::net::protocol::{InputEvent, ServerMsg};
use prfh::net::server::{send_msg, spawn_listener, HostEvent, HostState, HOST_ID};

// Drive the host event loop manually for a short while in a background thread.
#[test]
fn client_sees_host_keystroke() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    // Host thread: processes events + host's own input.
    let host_handle = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
        let mut conn_player: HashMap<u64, u8> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut typed = false;
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    HostEvent::Hello {
                        conn_id,
                        name,
                        mut write,
                    } => {
                        let outcome = host.add_player(name).unwrap();
                        send_msg(&mut write, &outcome.welcome).unwrap();
                        conn_player.insert(conn_id, outcome.id);
                        streams.insert(conn_id, write);
                        // After a client joins, host types 'h' once.
                        if !typed {
                            if let Some(msg) = host.apply_input(HOST_ID, InputEvent::Char('h')) {
                                for s in streams.values_mut() {
                                    send_msg(s, &msg).unwrap();
                                }
                            }
                            typed = true;
                        }
                    }
                    HostEvent::Input { conn_id, ev } => {
                        if let Some(&pid) = conn_player.get(&conn_id) {
                            if let Some(msg) = host.apply_input(pid, ev) {
                                for s in streams.values_mut() {
                                    let _ = send_msg(s, &msg);
                                }
                            }
                        }
                    }
                    HostEvent::Disconnected { .. } => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (world, _arena, handle): (WorldView, _, _) = connect(&addr.to_string(), "Bob").unwrap();
    assert!(world.players.iter().any(|p| p.is_self)); // got Welcome

    // Wait for the host's 'h' keystroke to arrive.
    let msg = handle.rx.recv_timeout(Duration::from_secs(3)).unwrap();
    match msg {
        ServerMsg::Wrote { id, tile, .. } => {
            assert_eq!(id, HOST_ID);
            assert_eq!(tile.ch, 'h');
        }
        other => panic!("expected Wrote, got {:?}", other),
    }

    drop(handle);
    let _ = host_handle.join();
}
