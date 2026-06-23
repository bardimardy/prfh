use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use prfh::game::arena::{Arena, EntityKind, PowerupWord};
use prfh::net::client::connect;
use prfh::net::protocol::ServerMsg;
use prfh::net::server::{spawn_listener, HostEvent, HostState};

fn powerup(word: &str) -> EntityKind {
    EntityKind::PowerupWord(PowerupWord { word: word.into() })
}

/// (a) Delta-Pfad: Host spawnt eine Entität, NACHDEM der Client verbunden ist.
/// Der Client muss das `EntitySpawned`-Delta empfangen und in seine Arena-Kopie
/// anwenden.
#[test]
fn client_sees_entity_spawned_via_delta() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    // Host-Thread: akzeptiert den Client, spawnt dann eine Entität und
    // broadcastet das Delta.
    let host = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut spawned = false;
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                if let HostEvent::Hello {
                    conn_id,
                    name,
                    mut write,
                } = ev
                {
                    let outcome = host.add_player(name).unwrap();
                    prfh::net::server::send_msg(&mut write, &outcome.welcome).unwrap();
                    streams.insert(conn_id, write);
                }
            }
            // Sobald ein Client da ist, einmalig spawnen + broadcasten.
            if !spawned && !streams.is_empty() {
                let msg = host.spawn_entity((7, 3), powerup("rebase"));
                for s in streams.values_mut() {
                    let _ = prfh::net::server::send_msg(s, &msg);
                }
                spawned = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (_world, mut arena, handle) = connect(&addr.to_string(), "Bob").unwrap();
    assert!(arena.entities.is_empty(), "frischer Client startet ohne Entitäten");

    // Auf das Delta warten und anwenden.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut got = false;
    while Instant::now() < deadline && !got {
        if let Ok(ServerMsg::EntitySpawned { entity }) =
            handle.rx.recv_timeout(Duration::from_millis(200))
        {
            arena.apply_spawned(entity);
            got = true;
        }
    }
    assert!(got, "Client hat kein EntitySpawned-Delta empfangen");
    assert_eq!(arena.entities.len(), 1);
    assert_eq!(arena.entities[0].pos, (7, 3));

    drop(handle);
    let _ = host.join();
}

/// (b) Late-Join-Pfad: Host spawnt eine Entität, BEVOR der Client verbindet.
/// Der `Welcome`-Snapshot muss die Entität tragen, sodass `connect` eine
/// vorbefüllte Arena liefert.
#[test]
fn late_join_client_gets_entity_via_welcome_snapshot() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    let host = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        // VOR dem Accept spawnen → landet im Welcome-Snapshot.
        host.spawn_entity((1, 2), powerup("sudo"));
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                if let HostEvent::Hello {
                    name, mut write, ..
                } = ev
                {
                    let outcome = host.add_player(name).unwrap();
                    prfh::net::server::send_msg(&mut write, &outcome.welcome).unwrap();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (_world, arena, _handle): (_, Arena, _) = connect(&addr.to_string(), "Late").unwrap();
    assert_eq!(arena.entities.len(), 1, "Late-Join muss die Entität via Snapshot sehen");
    assert_eq!(arena.entities[0].pos, (1, 2));
    match &arena.entities[0].kind {
        EntityKind::PowerupWord(pw) => assert_eq!(pw.word, "sudo"),
    }

    let _ = host.join();
}
