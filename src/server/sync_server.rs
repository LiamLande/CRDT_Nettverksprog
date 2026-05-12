use crate::server::OperationStore;
use crate::sync::Operation;
use crate::sync::protocol::{ClientWireMessage, ServerWireMessage};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

pub fn run_sync_server(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let store = Arc::new(Mutex::new(OperationStore::default()));
    let peers = Arc::new(Mutex::new(Vec::<mpsc::Sender<Operation>>::new()));

    println!("BetSync sync server listening on {addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let store = Arc::clone(&store);
                let peers = Arc::clone(&peers);
                thread::spawn(move || {
                    if let Err(error) = handle_client(stream, store, peers) {
                        eprintln!("client disconnected: {error}");
                    }
                });
            }
            Err(error) => eprintln!("failed to accept connection: {error}"),
        }
    }
    Ok(())
}

fn handle_client(
    stream: TcpStream,
    store: Arc<Mutex<OperationStore>>,
    peers: Arc<Mutex<Vec<mpsc::Sender<Operation>>>>,
) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    let mut hello_line = String::new();
    if reader.read_line(&mut hello_line)? == 0 {
        return Ok(());
    }

    let hello = serde_json::from_str::<ClientWireMessage>(hello_line.trim_end())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let (replica_id, seen_operations) = match hello {
        ClientWireMessage::Hello {
            replica_id,
            seen_operations,
        } => (replica_id, seen_operations),
        ClientWireMessage::Operation(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "first message must be Hello",
            ));
        }
    };

    let missing = store.lock().unwrap().missing_for(&seen_operations);
    write_server_message(
        &mut writer,
        &ServerWireMessage::Info {
            message: format!(
                "connected as {replica_id}; sending {} missing operations",
                missing.len()
            ),
        },
    )?;
    for operation in missing {
        write_server_message(&mut writer, &ServerWireMessage::Operation(operation))?;
    }

    let (tx, rx) = mpsc::channel::<Operation>();
    peers.lock().unwrap().push(tx);

    let mut peer_writer = writer.try_clone()?;
    thread::spawn(move || {
        for operation in rx {
            if write_server_message(&mut peer_writer, &ServerWireMessage::Operation(operation))
                .is_err()
            {
                break;
            }
        }
    });

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let message = serde_json::from_str::<ClientWireMessage>(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let ClientWireMessage::Operation(operation) = message else {
            continue;
        };

        let is_new = store.lock().unwrap().insert(operation.clone());
        if is_new {
            let operation_count = store.lock().unwrap().len();
            println!(
                "accepted operation {} from {}; log size {}",
                operation.op_id, operation.replica_id, operation_count
            );
            for peer in peers.lock().unwrap().iter() {
                let _ = peer.send(operation.clone());
            }
        }
    }

    Ok(())
}

fn write_server_message(stream: &mut TcpStream, message: &ServerWireMessage) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stream.write_all(b"\n")?;
    stream.flush()
}
