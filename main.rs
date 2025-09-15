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

type SharedConnections = Arc<Mutex<HashMap<String, TcpStream>>>;

#[derive(Clone, Copy)]
struct ServerConfig {
    port: i32,
    no_mirror: bool,
    max_players: i32,
    max_rate: i32
}

// xxxx-xxxx
fn generate_id() -> String {
    let mut rng = rand::rng();
    (0..8)
        .map(|i| {
            if i == 4 {
                '-'
            } else {
                let n = rng.random_range(0..16);
                std::char::from_digit(n, 16).unwrap()
            }
        })
        .collect()
}

fn handle_client(mut stream: TcpStream, mut id: String, connections: SharedConnections, config: ServerConfig, running: Arc<AtomicBool>) {
    println!("INFO:: {} connected", id);

    {
        let mut conns = connections.lock().unwrap();
        conns.insert(id.clone(), stream.try_clone().unwrap());
    }

    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

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
                let key = buffer[4];
                let content = &buffer[5..size as usize];

                // handle join and leave
                if key == 1 { // join                    
                    let new_id = String::from_utf8_lossy(&content[0..64]).trim().to_string();

                    println!("INFO:: {} changed id to {}.", id, new_id);

                    connections.lock().unwrap().remove(&id);
                    connections.lock().unwrap().insert(new_id.clone(), stream.try_clone().unwrap());

                    id = new_id;
                }
                if key == 2 { // leave
                    println!("INFO:: {} left.", id);
                }

                println!("INFO:: Broadcasting packet with key {} for id {}", key, id);

                // broadcast
                let conns = connections.lock().unwrap();
                for (other_id, mut conn) in conns.iter() {
                    if other_id != &id || !config.no_mirror {
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
        let mut conns = connections.lock().unwrap();
        conns.remove(&id);
    }

    {
        let mut leave_msg = [0u8; 69];
        leave_msg[0..4].copy_from_slice(&(69 as i32).to_le_bytes());
        leave_msg[5] = 2;
        leave_msg[6..69].copy_from_slice(id.as_bytes());
        
        let conns = connections.lock().unwrap();
        for (_, mut conn) in conns.iter() {
            let _ = conn.write_all(&leave_msg);
        }
    }

    println!("INFO:: {} disconnected", id);
}

fn main() {
    let mut config = ServerConfig { port: 45565, no_mirror: false, max_players: 10, max_rate: 60 };
    let args: Vec<String> = env::args().skip(1).collect();
    for arg in &args {
        if arg == "--no-mirror" {
            config.no_mirror = true;
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

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address).unwrap();
    listener.set_nonblocking(true).unwrap();

    println!("INFO:: listening on port {} with the following configuration:", config.port);
    println!("INFO:: mirror           = {}", if config.no_mirror { "disabled" } else { "enabled" });
    println!("INFO:: max players      = {}", config.max_players);
    println!("INFO:: max message rate = {}", config.max_rate);
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

                let tmp_id = generate_id();
                let running = Arc::clone(&running);
                let conns = Arc::clone(&connections);

                thread::spawn(move || handle_client(stream, tmp_id, conns, config, running));
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
