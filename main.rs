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

fn handle_client(stream: TcpStream, connections: SharedConnections, config: ServerConfig, running: Arc<AtomicBool>) {
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

    let mut msg_times = VecDeque::<Instant>::new();

    loop {
        // read size
        let mut size_bytes = [0u8; 4];
        match read_bytes(&stream, &mut size_bytes, 4, &running) {
            Ok(_) => { },
            Err(e) => match e {
                Some(e) => {
                    eprintln!("ERROR:: {} - Encountered error {}, closing thread!", id, e);
                    break;
                },
                None => { }
            }
        };
        
        let size_bytes_4: [u8; 4] = match size_bytes.try_into() {
            Ok(b) => b,
            Err(_) => {
                eprintln!("ERROR:: {} - Failed to convert size bytes, closing thread!", id);
                break;
            }
        };
        
        let size = i32::from_le_bytes(size_bytes_4);

        if size > 512 {
            eprintln!("ERROR:: {} - Packet too large ({}), closing thread!", id, size);
            break;
        }

        if size < 4 {
            eprintln!("ERROR:: {} - Packet too small ({}), closing thread!", id, size);
            break;
        }

        // read content
        let content_size = (size - 4) as usize;

        let mut content_bytes = vec![0u8; content_size];
        match read_bytes(&stream, &mut content_bytes, content_size, &running) {
            Ok(_) => { },
            Err(e) => match e {
                Some(e) => {
                    eprintln!("ERROR:: {} - Encountered error {}, closing thread!", id, e);
                    break;
                },
                None => { }
            }
        };

        { // throttle
            let now = Instant::now();

            while let Some(&t) = msg_times.front() {
                if now.duration_since(t).as_secs_f64() > 1.0 {
                    msg_times.pop_front();
                } else { break; }
            }

            if msg_times.len() as i32 >= config.max_rate { continue; }
            msg_times.push_back(now);
        }

        { // broadcast
            if config.debug_print {
                println!("INFO:: {} - Broadcasting packet of size {}.", id, size);
            }

            let _connections = match connections.lock() {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("ERROR:: Could not lock connections, closing thread!");
                    break;
                }
            };

            for (other_id, mut conn) in _connections.iter() {
                if other_id != &id || config.mirror {
                    conn.write_all(&size_bytes).expect("could not send size");
                    conn.write_all(&content_bytes).expect("could not send content");
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

fn read_bytes(mut stream: &TcpStream, buffer: &mut [u8], length: usize, running: &Arc<AtomicBool>) -> Result<(), Option<std::io::Error>> {
    let mut read = 0;

    while read < length {
        if !running.load(Ordering::SeqCst) { return Err(None); }

        let mut buf = vec![0u8; length - read];

        match stream.read(&mut buf) {
            Ok(0) => {
                return Err(None);
            },
            Ok(n) => {
                buffer[read..(read + n)].clone_from_slice(&buf[0..n]);
                read += n;
            },
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }

                return Err(Some(e));
            }
        }
    }

    Ok(())
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
