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
    let id = rand::rng().random_range(0..16384);

    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    {
        connections.lock().unwrap().insert(id, stream.try_clone().unwrap());
        println!("INFO:: {} connected", id);
    }

    let mut buffer = [0u8; 512];
    let mut msg_times = VecDeque::<Instant>::new();

    while running.load(Ordering::SeqCst) {
        match stream.read(&mut buffer) {
            Ok(0) => { break; },
            Ok(n) => {
                if n <= 0 { continue; }

                // throttle
                let now = Instant::now();
                while let Some(&t) = msg_times.front() {
                    if now.duration_since(t).as_secs_f64() > 1.0 {
                        msg_times.pop_front();
                    } else { break; }
                }
                if msg_times.len() as i32 >= config.max_rate { continue; }
                msg_times.push_back(now);

                // read data
                let size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());

                if config.debug_print {
                    println!("INFO:: Broadcasting packet of size {} for id {}", size, id);
                }

                // broadcast
                let conns = connections.lock().unwrap();
                for (other_id, mut conn) in conns.iter() {
                    if other_id != &id || config.mirror {
                        let _ = conn.write_all(&buffer[0..size as usize]);
                    }
                }
            },
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock && e.kind() != ErrorKind::TimedOut {
                    eprintln!("ERROR:: Connection error: {}", e);
                    break;
                }
            }
        }
    }

    {
        connections.lock().unwrap().remove(&id);
        println!("INFO:: {} disconnected", id);
    }
}

fn main() {
    let config = read_config();

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address).unwrap();
    listener.set_nonblocking(true).unwrap();

    println!("INFO:: listening on port {} with the following configuration:", config.port);
    println!("INFO:: mirror           = {}", if config.mirror { "enabled" } else { "disabled" });
    println!("INFO:: max players      = {}", config.max_players);
    println!("INFO:: max message rate = {}", config.max_rate);
    println!("INFO:: debug logging    = {}", if config.debug_print { "enabled" } else { "disabled" });
    println!();

    let connections: SharedConnections = Arc::new(Mutex::new(HashMap::new()));
    let running = Arc::new(AtomicBool::new(true));

    {
        let running = Arc::clone(&running);
        ctrlc::set_handler(move || {
            println!("\nINFO:: Shutdown signal received.");
            running.store(false, Ordering::SeqCst);
        }).expect("ERROR:: Failed to set Ctrl+C handler");
    }

    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if connections.lock().unwrap().len() as i32 >= config.max_players { continue; }

                let running = Arc::clone(&running);
                let conns = Arc::clone(&connections);

                thread::spawn(move || handle_client(stream, conns, config, running));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                eprintln!("ERROR:: Connection error: {}", e);
            }
        }
    }

    println!("INFO:: Server shutting down. Closing all connections...");
    let conns = connections.lock().unwrap();
    for (id, conn) in conns.iter() {
        let _ = conn.shutdown(std::net::Shutdown::Both);
        println!("INFO:: Closed connection for {}", id);
    }

    println!("INFO:: Shutdown complete.");
}
