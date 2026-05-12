use anyhow::{Context, Result};
use betsync::client::LocalClient;
use betsync::crdt::PNCounter;
use betsync::domain::BetKind;
use betsync::frontend::run_frontend_server;
use betsync::server::run_sync_server;
use betsync::sync::Operation;
use betsync::sync::protocol::{ClientWireMessage, ServerWireMessage};
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Debug, Parser)]
#[command(name = "betsync")]
#[command(about = "Distributed fake-chip betting simulator using custom CRDTs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Server {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
    Client {
        #[arg(long)]
        replica_id: String,
        #[arg(long)]
        server: Option<String>,
    },
    Simulate {
        #[arg(value_enum)]
        scenario: Scenario,
    },
    Frontend {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum Scenario {
    OfflineMerge,
    DoubleSpend,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Server { host, port } => {
            run_sync_server(&format!("{host}:{port}")).map_err(Into::into)
        }
        Commands::Client { replica_id, server } => run_client(replica_id, server),
        Commands::Simulate { scenario } => match scenario {
            Scenario::OfflineMerge => simulate_offline_merge(),
            Scenario::DoubleSpend => simulate_double_spend(),
        },
        Commands::Frontend { host, port } => run_frontend_server(&format!("{host}:{port}")),
    }
}

struct ActiveConnection {
    writer: Arc<Mutex<TcpStream>>,
    incoming: mpsc::Receiver<Operation>,
}

impl ActiveConnection {
    fn connect(server: &str, client: &LocalClient) -> Result<Self> {
        let addr = normalize_server_addr(server);
        let stream = TcpStream::connect(&addr).with_context(|| format!("connect to {addr}"))?;
        let writer = Arc::new(Mutex::new(stream.try_clone()?));
        write_client_message(
            &mut writer.lock().unwrap(),
            &ClientWireMessage::Hello {
                replica_id: client.replica_id.clone(),
                seen_operations: client.log.ids(),
            },
        )?;

        let (tx, incoming) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<ServerWireMessage>(&line) {
                    Ok(ServerWireMessage::Operation(operation)) => {
                        if tx.send(operation).is_err() {
                            break;
                        }
                    }
                    Ok(ServerWireMessage::Info { message }) => println!("server: {message}"),
                    Err(error) => eprintln!("invalid server message: {error}"),
                }
            }
        });

        Ok(Self { writer, incoming })
    }

    fn send(&self, operation: &Operation) -> Result<()> {
        write_client_message(
            &mut self.writer.lock().unwrap(),
            &ClientWireMessage::Operation(operation.clone()),
        )
    }
}

fn run_client(replica_id: String, server: Option<String>) -> Result<()> {
    let mut client = LocalClient::new(replica_id);
    let mut current_player: Option<String> = None;
    let mut current_round: Option<String> = None;
    let mut connection = match &server {
        Some(server) => match ActiveConnection::connect(server, &client) {
            Ok(connection) => {
                println!("connected to {}", normalize_server_addr(server));
                Some(connection)
            }
            Err(error) => {
                eprintln!("starting offline: {error}");
                client.set_online(false);
                None
            }
        },
        None => {
            client.set_online(false);
            None
        }
    };

    print_help();
    let stdin = io::stdin();
    loop {
        drain_incoming(&mut client, &connection);
        print!("betsync:{}> ", client.replica_id);
        io::stdout().flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "help" => print_help(),
            "quit" | "exit" => break,
            "offline" => {
                client.set_online(false);
                connection = None;
                println!("offline mode enabled");
            }
            "online" => {
                let Some(server) = &server else {
                    println!("no --server was configured");
                    continue;
                };
                match ActiveConnection::connect(server, &client) {
                    Ok(new_connection) => {
                        client.set_online(true);
                        let unsynced = client.take_unsynced();
                        for operation in &unsynced {
                            new_connection.send(operation)?;
                        }
                        println!("online; sent {} queued operations", unsynced.len());
                        connection = Some(new_connection);
                    }
                    Err(error) => {
                        client.set_online(false);
                        println!("could not reconnect: {error}");
                    }
                }
            }
            "create-table" => {
                let name = parts.get(1).copied().unwrap_or("BetSync Table");
                let operation = client.create_table(name);
                publish(&mut client, &connection, &operation)?;
            }
            "join" => {
                let display_name = parts.get(1).copied().unwrap_or("Player");
                let player_id = parts
                    .get(2)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| display_name.to_ascii_lowercase());
                let operation = client.join_table(player_id.clone(), display_name, 100);
                current_player = Some(player_id);
                publish(&mut client, &connection, &operation)?;
            }
            "join-split" => {
                if parts.len() < 5 {
                    println!("usage: join-split <player_id> <display_name> <replica:rights>...");
                    continue;
                }
                let player_id = parts[1].to_string();
                let display_name = parts[2].to_string();
                let mut rights = BTreeMap::new();
                for entry in &parts[3..] {
                    let Some((replica, amount)) = entry.split_once(':') else {
                        println!("rights entry must look like A:60");
                        continue;
                    };
                    let amount = amount.parse::<u64>().context("parse rights amount")?;
                    rights.insert(replica.to_string(), amount);
                }
                let starting_chips = rights.values().sum();
                let operation = client.join_table_with_rights(
                    player_id.clone(),
                    display_name,
                    starting_chips,
                    rights,
                );
                current_player = Some(player_id);
                publish(&mut client, &connection, &operation)?;
            }
            "start-round" => {
                let round_id = parts
                    .get(1)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| format!("round-{}", client.clock.now() + 1));
                let dealer_id = current_player
                    .clone()
                    .unwrap_or_else(|| client.replica_id.clone());
                let operations = client.start_round(round_id.clone(), dealer_id);
                current_round = Some(round_id);
                publish_many(&mut client, &connection, &operations)?;
            }
            "bet" => {
                if parts.len() < 3 {
                    println!(
                        "usage: bet <odd|even|high|low|exactN> <amount> [player_id] [round_id]"
                    );
                    continue;
                }
                let bet_kind = BetKind::from_str(parts[1]).map_err(anyhow::Error::msg)?;
                let amount = parts[2].parse::<u64>().context("parse bet amount")?;
                let Some(player_id) = parts
                    .get(3)
                    .map(|value| value.to_string())
                    .or_else(|| current_player.clone())
                else {
                    println!("join first or pass player_id");
                    continue;
                };
                let Some(round_id) = parts
                    .get(4)
                    .map(|value| value.to_string())
                    .or_else(|| current_round.clone())
                    .or_else(|| client.state.active_round_id())
                else {
                    println!("start a round first or pass round_id");
                    continue;
                };
                match client.place_bet(player_id, round_id, amount, bet_kind) {
                    Ok(operation) => publish(&mut client, &connection, &operation)?,
                    Err(error) => println!("bet rejected: {error}"),
                }
            }
            "close-round" => {
                let Some(round_id) = parts
                    .get(1)
                    .map(|value| value.to_string())
                    .or_else(|| current_round.clone())
                    .or_else(|| client.state.active_round_id())
                else {
                    println!("usage: close-round [round_id]");
                    continue;
                };
                match client.close_betting(round_id) {
                    Ok(operation) => publish(&mut client, &connection, &operation)?,
                    Err(error) => println!("close rejected: {error}"),
                }
            }
            "resolve" => {
                if parts.len() < 2 {
                    println!("usage: resolve <dice 1-6> [round_id]");
                    continue;
                }
                let dice = parts[1].parse::<u8>().context("parse dice result")?;
                let Some(round_id) = parts
                    .get(2)
                    .map(|value| value.to_string())
                    .or_else(|| current_round.clone())
                    .or_else(|| client.state.active_round_id())
                else {
                    println!("usage: resolve <dice 1-6> [round_id]");
                    continue;
                };
                match client.resolve_round(round_id, dice) {
                    Ok(operations) => publish_many(&mut client, &connection, &operations)?,
                    Err(error) => println!("resolve rejected: {error}"),
                }
            }
            "transfer-rights" => {
                if parts.len() < 4 {
                    println!("usage: transfer-rights <player_id> <to_replica> <amount>");
                    continue;
                }
                let amount = parts[3].parse::<u64>().context("parse rights amount")?;
                match client.transfer_spending_rights(parts[1], parts[2], amount) {
                    Ok(operation) => publish(&mut client, &connection, &operation)?,
                    Err(error) => println!("transfer rejected: {error}"),
                }
            }
            "balance" => {
                let Some(player_id) = parts
                    .get(1)
                    .map(|value| value.to_string())
                    .or_else(|| current_player.clone())
                else {
                    println!("usage: balance [player_id]");
                    continue;
                };
                println!(
                    "{player_id} balance: {}",
                    client.state.player_balance(&player_id)
                );
            }
            "rights" => {
                let Some(player_id) = parts
                    .get(1)
                    .map(|value| value.to_string())
                    .or_else(|| current_player.clone())
                else {
                    println!("usage: rights [player_id]");
                    continue;
                };
                print_rights(&client, &player_id);
            }
            "state" | "show-state" => {
                println!("{}", serde_json::to_string_pretty(&client.state)?);
            }
            "log" | "show-log" => {
                for operation in client.log.sorted_operations() {
                    println!(
                        "{} @{} {} {:?}",
                        operation.op_id,
                        operation.lamport_time,
                        operation.replica_id,
                        operation.kind
                    );
                }
            }
            "resend" => {
                let Some(op_id) = parts.get(1) else {
                    println!("usage: resend <op_id>");
                    continue;
                };
                let Some(operation) = client.operation_by_id(op_id) else {
                    println!("unknown operation {op_id}");
                    continue;
                };
                publish(&mut client, &connection, &operation)?;
                println!("resent {}", operation.op_id);
            }
            "simulate-double-spend" => simulate_double_spend()?,
            "simulate-offline-merge" => simulate_offline_merge()?,
            other => println!("unknown command {other}; try help"),
        }
    }

    Ok(())
}

fn publish(
    client: &mut LocalClient,
    connection: &Option<ActiveConnection>,
    operation: &Operation,
) -> Result<()> {
    if client.is_online() {
        if let Some(connection) = connection {
            if let Err(error) = connection.send(operation) {
                client.queue_unsynced(operation.clone());
                client.set_online(false);
                eprintln!("send failed; operation queued for reconnect: {error}");
            }
        }
    }
    Ok(())
}

fn publish_many(
    client: &mut LocalClient,
    connection: &Option<ActiveConnection>,
    operations: &[Operation],
) -> Result<()> {
    for operation in operations {
        publish(client, connection, operation)?;
    }
    Ok(())
}

fn drain_incoming(client: &mut LocalClient, connection: &Option<ActiveConnection>) {
    let Some(connection) = connection else {
        return;
    };
    let mut count = 0;
    while let Ok(operation) = connection.incoming.try_recv() {
        if client.ingest(operation) {
            count += 1;
        }
    }
    if count > 0 {
        println!("merged {count} remote operations");
    }
}

fn print_rights(client: &LocalClient, player_id: &str) {
    let Some(counter) = client.state.spending_rights.get(player_id) else {
        println!("{player_id} has no spending-right counter");
        return;
    };
    let mut replicas: Vec<_> = counter.initial_rights.keys().cloned().collect();
    replicas.extend(
        counter
            .grants
            .values()
            .map(|grant| grant.replica_id.clone()),
    );
    replicas.extend(
        counter
            .transfers
            .values()
            .flat_map(|transfer| [transfer.from_replica.clone(), transfer.to_replica.clone()]),
    );
    replicas.sort();
    replicas.dedup();
    for replica_id in replicas {
        println!(
            "{player_id}@{replica_id}: {} rights",
            counter.available(&replica_id)
        );
    }
    println!("total spent: {}", counter.total_spent());
}

fn print_help() {
    println!(
        "commands: help, create-table <name>, join <display> [player_id], join-split <player> <display> A:60 B:40, start-round [id], bet <kind> <amount>, close-round, resolve <dice>, balance, rights, state, log, offline, online, resend <op_id>, quit"
    );
}

fn write_client_message(stream: &mut TcpStream, message: &ClientWireMessage) -> Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn normalize_server_addr(server: &str) -> String {
    if server.contains(':') {
        server.to_string()
    } else {
        format!("{server}:8080")
    }
}

fn simulate_offline_merge() -> Result<()> {
    let mut rights = BTreeMap::new();
    rights.insert("A".to_string(), 60);
    rights.insert("B".to_string(), 40);

    let mut a = LocalClient::new("A");
    a.create_table("BetSync Demo");
    a.join_table_with_rights("liam", "Liam", 100, rights);
    a.start_round("round-1", "dealer");

    let mut b = LocalClient::new("B");
    b.ingest_many(a.log.operations.clone());

    a.set_online(false);
    b.set_online(false);
    let a_bet = a
        .place_bet("liam", "round-1", 50, BetKind::Odd)
        .map_err(anyhow::Error::msg)?;
    let b_bet = b
        .place_bet("liam", "round-1", 30, BetKind::Even)
        .map_err(anyhow::Error::msg)?;

    a.ingest_many(b.log.operations.clone());
    b.ingest_many(a.log.operations.clone());

    println!("offline merge simulation");
    println!("A offline op: {}", a_bet.op_id);
    println!("B offline op: {}", b_bet.op_id);
    println!("A bets: {}", a.state.visible_bets().len());
    println!("B bets: {}", b.state.visible_bets().len());
    println!("A balance: {}", a.state.player_balance("liam"));
    println!("B balance: {}", b.state.player_balance("liam"));
    println!("states equal: {}", a.state == b.state);
    Ok(())
}

fn simulate_double_spend() -> Result<()> {
    let mut base = PNCounter::default();
    base.increment("bank", 100);
    let mut pn_a = base.clone();
    let mut pn_b = base;
    pn_a.decrement("A", 80);
    pn_b.decrement("B", 80);
    let merged_pn = pn_a.merge(&pn_b);

    println!("PN-counter only failure");
    println!("starting balance: 100");
    println!("A offline spends 80, B offline spends 80");
    println!("merged PN-counter balance: {}", merged_pn.value());

    let mut rights = BTreeMap::new();
    rights.insert("A".to_string(), 60);
    rights.insert("B".to_string(), 40);

    let mut a = LocalClient::new("A");
    a.create_table("Bounded Counter Demo");
    a.join_table_with_rights("liam", "Liam", 100, rights);
    a.start_round("round-1", "dealer");

    let mut b = LocalClient::new("B");
    b.ingest_many(a.log.operations.clone());

    let a_rejected = a.place_bet("liam", "round-1", 80, BetKind::Odd).is_err();
    let b_rejected = b.place_bet("liam", "round-1", 80, BetKind::Even).is_err();
    a.place_bet("liam", "round-1", 50, BetKind::Odd)
        .map_err(anyhow::Error::msg)?;
    b.place_bet("liam", "round-1", 30, BetKind::Even)
        .map_err(anyhow::Error::msg)?;
    a.ingest_many(b.log.operations.clone());
    b.ingest_many(a.log.operations.clone());

    println!();
    println!("bounded counter solution");
    println!("A rights: 60, B rights: 40");
    println!("A tries 80 -> rejected: {a_rejected}");
    println!("B tries 80 -> rejected: {b_rejected}");
    println!("merged balance: {}", a.state.player_balance("liam"));
    println!(
        "remaining rights A/B: {}/{}",
        a.state.available_rights("liam", "A"),
        a.state.available_rights("liam", "B")
    );
    println!("states equal: {}", a.state == b.state);
    Ok(())
}
