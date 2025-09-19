use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use rand::Rng;

type SharedConnections = Arc<Mutex<HashMap<i32, TcpStream>>>;

#[derive(Clone, Copy)]
struct ServerConfig {
    port: i32,
    mirror: bool,
    max_players: i32,
    max_rate: i32,
    debug_print: bool
}

fn read_config() -> ServerConfig {
    let mut config = ServerConfig { port: 45565, mirror: true, max_players: 10, max_rate: 60, debug_print: false };
    let args: Vec<String> = env::args().skip(1).collect();
    for arg in &args {
        if arg == "--no-mirror" {
            config.mirror = false;
        } else if arg == "--debug" {
            config.debug_print = true;
        } else if let Ok(p) = arg.parse::<i32>() {
            config.port = p;
        } else if arg.starts_with("--max-players=") {
            if let Some(v) = arg.split("=").nth(1) {
                if let Ok(n) = v.parse::<i32>() {
                    config.max_players = n;
                }
            }
        } else if arg.starts_with("--max-rate=") {
            if let Some(v) = arg.split("=").nth(1) {
                if let Ok(n) = v.parse::<i32>() {
                    config.max_rate = n;
                }
            }
        }
    }

    return config;
}

fn handle_client(mut stream: TcpStream, connections: SharedConnections, config: ServerConfig, running: Arc<AtomicBool>) {
    let id = { // roll id
        let mut _id = 0;

        let conns = match connections.lock() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("ERROR:: Could not lock connections, closing thread!");
                return;
            }
        };

        while _id == 0 || conns.contains_key(&_id) { _id = rand::rng().random_range(10000..16384); }
        _id
    };

    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    { // add to connections
        let mut _connections = match connections.lock() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("ERROR:: Could not lock connections, closing thread!");
                return;
            }
        };

        let mut _stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => {
                eprintln!("ERROR:: Could not clone stream, closing thread!");
                return;
            }
        };

        _connections.insert(id, _stream);
        println!("INFO:: {} - Joined.", id);
    }

    let mut buffer = [0u8; 512];
    let mut msg_times = VecDeque::<Instant>::new();

    while running.load(Ordering::SeqCst) {
        match stream.read(&mut buffer) {
            Ok(0) => { break; },
            Ok(n) => {
                if n < 4 { continue; }

                // throttle
                let now = Instant::now();
                msg_times.push_back(now);

                while let Some(&t) = msg_times.front() {
                    if now.duration_since(t).as_secs_f64() > 1.0 {
                        msg_times.pop_front();
                    } else { break; }
                }

                if msg_times.len() as i32 >= config.max_rate { continue; }

                let size = { // read size
                    let size_bytes: [u8; 4] = match buffer[0..4].try_into() {
                        Ok(b) => b,
                        Err(_) => {
                            eprintln!("ERROR:: {} - Could not read size bytes, closing thread!", id);
                            break;
                        }
                    };

                    i32::from_le_bytes(size_bytes)
                };

                if size >= buffer.len() as i32 {
                    eprintln!("ERROR:: {} - Packet too large, closing connection, closing thread!", id);
                    break;
                }

                if config.debug_print {
                    println!("INFO:: {} - Broadcasting packet of size {}.", id, size);
                }

                { // broadcast
                    let _connections = match connections.lock() {
                        Ok(c) => c,
                        Err(_) => {
                            eprintln!("ERROR:: Could not lock connections, closing thread!");
                            break;
                        }
                    };
    
                    for (other_id, mut conn) in _connections.iter() {
                        if other_id != &id || config.mirror {
                            let _ = conn.write_all(&buffer[0..size as usize]);
                        }
                    }
                }
            },
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock && e.kind() != ErrorKind::TimedOut {
                    eprintln!("ERROR:: {} - Encountered error: {}, closing thread!", id, e);
                    break;
                }
            }
        }
    }

    { // remove from connections
        let mut _connections = match connections.lock() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("ERROR:: Could not lock connections, closing thread!");
                return;
            }
        };

        _connections.remove(&id);
        println!("INFO:: {} - Disconnected.", id);
    }
}

fn main() {
    let config = read_config();

    let address = format!("0.0.0.0:{}", config.port);
    let listener = match TcpListener::bind(address) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("ERROR:: Could not bind listener, exiting!");
            return;
        }
    };

    let _ = listener.set_nonblocking(true);

    println!("INFO:: Listening on port {} with the following configuration:", config.port);
    println!("INFO:: Mirror           = {}", if config.mirror { "enabled" } else { "disabled" });
    println!("INFO:: Max players      = {}", config.max_players);
    println!("INFO:: Max message rate = {}", config.max_rate);
    println!("INFO:: Debug logging    = {}", if config.debug_print { "enabled" } else { "disabled" });
    println!();

    let connections: SharedConnections = Arc::new(Mutex::new(HashMap::new()));
    let running = Arc::new(AtomicBool::new(true));

    { // setup ctrl+c listener
        let running = Arc::clone(&running);
        ctrlc::set_handler(move || {
            println!("\nINFO:: Shutdown signal received, exiting.");
            running.store(false, Ordering::SeqCst);
        }).expect("ERROR:: Failed to set Ctrl+C handler");
    }

    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let _connections = match connections.lock() {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("ERROR:: Could not lock connections, exiting!");
                        break;
                    }
                };

                if _connections.len() as i32 >= config.max_players { continue; }

                let running_clone = Arc::clone(&running);
                let connections_clone = Arc::clone(&connections);

                thread::spawn(move || handle_client(stream, connections_clone, config, running_clone));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                eprintln!("ERROR:: Encountered error {}, exiting!", e);
                break;
            }
        }
    }

    { // shut down
        println!("INFO:: Server shutting down. Closing all connections...");

        let _connections = match connections.lock() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("ERROR:: Could not lock connections, exiting!");
                return;
            }
        };

        for (_, conn) in _connections.iter() {
            let _ = conn.shutdown(std::net::Shutdown::Both);
        }

        println!("INFO:: Shutdown complete.");
    }
}
