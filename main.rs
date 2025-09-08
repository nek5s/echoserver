use std::collections::HashMap;
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
use rand::Rng;

type SharedConnections = Arc<Mutex<HashMap<String, TcpStream>>>;

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

fn handle_client(mut stream: TcpStream, id: String, connections: SharedConnections, no_mirror: bool, running: Arc<AtomicBool>) {
    println!("INFO:: {} connected", id);
    {
        let mut conns = connections.lock().unwrap();
        conns.insert(id.clone(), stream.try_clone().unwrap());
    }

    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    let mut buffer = [0u8; 512];
    let mut incoming_data = String::new();

    while running.load(Ordering::SeqCst) {
        match stream.read(&mut buffer) {
            Ok(0) => { break; },
            Ok(n) => {
                if n <= 0 {
                    continue;
                }

                incoming_data.push_str(&String::from_utf8_lossy(&buffer[..n]));

                while let Some(newline_index) = incoming_data.find('\n') {
                    let line = incoming_data[..newline_index].trim().to_string();
                    incoming_data = incoming_data[(newline_index + 1)..].to_string();

                    let player_count = {
                        let conns = connections.lock().unwrap();
                        conns.len()
                    };

                    let metadata = format!("{}", player_count);
                    let msg = format!("{}::{}::{}\n", id, metadata, line);

                    // println!("INFO:: Broadcasting packet for {}", id);

                    let conns = connections.lock().unwrap();
                    for (other_id, mut conn) in conns.iter() {
                        if !no_mirror || other_id != &id {
                            let _ = conn.write_all(msg.as_bytes());
                        }
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

    println!("INFO:: {} disconnected", id);
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut port = 45565;
    let mut no_mirror = false;

    for arg in &args {
        if arg == "--no-mirror" {
            no_mirror = true;
        } else if let Ok(p) = arg.parse::<u16>() {
            port = p;
        }
    }

    let address = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;

    if no_mirror {
        println!("INFO:: listening on port {} with mirror disabled.", port);
    } else {
        println!("INFO:: listening on port {}.", port);
    }

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
                let id = generate_id();
                let conns = Arc::clone(&connections);
                let running = Arc::clone(&running);
                thread::spawn(move || handle_client(stream, id, conns, no_mirror, running));
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
    Ok(())
}
