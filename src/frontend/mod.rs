use crate::client::LocalClient;
use crate::crdt::PNCounter;
use crate::domain::{Bet, BetKind, RoundPhase};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const INDEX_HTML: &str = include_str!("assets/index.html");
const APP_CSS: &str = include_str!("assets/styles.css");
const APP_JS: &str = include_str!("assets/app.js");

pub fn run_frontend_server(addr: &str) -> Result<()> {
    let server = Server::http(addr).map_err(|error| anyhow!("bind frontend server: {error}"))?;
    let demo = Arc::new(Mutex::new(FrontendDemo::new()));

    println!("BetSync frontend running at http://{addr}");
    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, Arc::clone(&demo)) {
            eprintln!("frontend request failed: {error}");
        }
    }

    Ok(())
}

fn handle_request(mut request: Request, demo: Arc<Mutex<FrontendDemo>>) -> Result<()> {
    let method = request.method().clone();
    let url = request.url().to_string();

    match (method, url.as_str()) {
        (Method::Get, "/") | (Method::Get, "/index.html") => respond(
            request,
            INDEX_HTML,
            "text/html; charset=utf-8",
            StatusCode(200),
        ),
        (Method::Get, "/styles.css") => {
            respond(request, APP_CSS, "text/css; charset=utf-8", StatusCode(200))
        }
        (Method::Get, "/app.js") => respond(
            request,
            APP_JS,
            "application/javascript; charset=utf-8",
            StatusCode(200),
        ),
        (Method::Get, "/api/state") => {
            let snapshot = demo.lock().unwrap().snapshot();
            respond_json(request, &snapshot, StatusCode(200))
        }
        (Method::Post, "/api/action") => {
            let action = parse_action(request.as_reader())?;
            let snapshot = demo.lock().unwrap().apply(action);
            respond_json(request, &snapshot, StatusCode(200))
        }
        _ => respond(
            request,
            "not found",
            "text/plain; charset=utf-8",
            StatusCode(404),
        ),
    }
}

fn parse_action(mut reader: impl Read) -> Result<ActionRequest> {
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .context("read action request")?;
    serde_json::from_str(&body).context("parse action request")
}

fn respond(request: Request, body: &str, content_type: &str, status: StatusCode) -> Result<()> {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(content_type_header(content_type)?);
    request
        .respond(response)
        .map_err(|error| anyhow!("send response: {error}"))
}

fn respond_json<T: Serialize>(request: Request, value: &T, status: StatusCode) -> Result<()> {
    let body = serde_json::to_string(value).context("serialize frontend response")?;
    respond(request, &body, "application/json; charset=utf-8", status)
}

fn content_type_header(content_type: &str) -> Result<Header> {
    Header::from_bytes("Content-Type", content_type)
        .map_err(|_| anyhow!("build content-type header"))
}

#[derive(Debug)]
struct FrontendDemo {
    a: LocalClient,
    b: LocalClient,
    messages: Vec<String>,
    pn_failure_balance: Option<i64>,
}

impl FrontendDemo {
    fn new() -> Self {
        let mut demo = Self {
            a: LocalClient::new("A"),
            b: LocalClient::new("B"),
            messages: Vec::new(),
            pn_failure_balance: None,
        };
        demo.reset();
        demo
    }

    fn reset(&mut self) {
        let mut rights = BTreeMap::new();
        rights.insert("A".to_string(), 60);
        rights.insert("B".to_string(), 40);

        self.a = LocalClient::new("A");
        self.b = LocalClient::new("B");
        self.a.create_table("BetSync Web Demo");
        self.a.join_table_with_rights("liam", "Liam", 100, rights);
        self.a.start_round("round-1", "dealer");
        self.b.ingest_many(self.a.log.operations.clone());
        self.messages.clear();
        self.pn_failure_balance = None;
        self.messages
            .push("demo reset: Liam has 100 chips split as A:60 and B:40".to_string());
    }

    fn apply(&mut self, action: ActionRequest) -> FrontendSnapshot {
        let result = match action.action.as_str() {
            "reset" => {
                self.reset();
                Ok("reset demo".to_string())
            }
            "sync" => {
                self.sync_all();
                Ok("merged operation logs between A and B".to_string())
            }
            "toggle-online" => self.toggle_online(action.replica.as_deref()),
            "bet" => self.place_bet(action),
            "close-round" => self.close_round(),
            "resolve-round" => self.resolve_round(action.dice.unwrap_or(5)),
            "pn-failure" => {
                self.show_pn_failure();
                Ok("PN-counter failure calculated".to_string())
            }
            other => Err(format!("unknown action {other}")),
        };

        match result {
            Ok(message) => self.messages.push(message),
            Err(error) => self.messages.push(format!("rejected: {error}")),
        }
        self.trim_messages();
        self.snapshot()
    }

    fn toggle_online(&mut self, replica_id: Option<&str>) -> Result<String, String> {
        let replica_id = replica_id.ok_or_else(|| "missing replica".to_string())?;
        match replica_id {
            "A" => {
                self.a.set_online(!self.a.is_online());
                if self.a.is_online() {
                    self.sync_if_connected();
                }
                Ok(format!("replica A is {}", online_label(self.a.is_online())))
            }
            "B" => {
                self.b.set_online(!self.b.is_online());
                if self.b.is_online() {
                    self.sync_if_connected();
                }
                Ok(format!("replica B is {}", online_label(self.b.is_online())))
            }
            _ => Err(format!("unknown replica {replica_id}")),
        }
    }

    fn place_bet(&mut self, action: ActionRequest) -> Result<String, String> {
        let replica_id = action.replica.as_deref().unwrap_or("A");
        let amount = action.amount.unwrap_or(10);
        let kind = action
            .bet_kind
            .as_deref()
            .unwrap_or("odd")
            .parse::<BetKind>()?;

        let operation = match replica_id {
            "A" => self.a.place_bet("liam", "round-1", amount, kind.clone()),
            "B" => self.b.place_bet("liam", "round-1", amount, kind.clone()),
            _ => return Err(format!("unknown replica {replica_id}")),
        }?;

        self.sync_if_connected();
        Ok(format!(
            "replica {replica_id} placed {amount} on {kind} ({})",
            operation.op_id
        ))
    }

    fn close_round(&mut self) -> Result<String, String> {
        let operation = self.a.close_betting("round-1")?;
        self.sync_if_connected();
        Ok(format!("replica A closed betting ({})", operation.op_id))
    }

    fn resolve_round(&mut self, dice: u8) -> Result<String, String> {
        let operations = self.a.resolve_round("round-1", dice)?;
        self.sync_if_connected();
        Ok(format!(
            "replica A resolved round with dice {dice}; emitted {} operations",
            operations.len()
        ))
    }

    fn show_pn_failure(&mut self) {
        let mut base = PNCounter::default();
        base.increment("bank", 100);
        let mut pn_a = base.clone();
        let mut pn_b = base;
        pn_a.decrement("A", 80);
        pn_b.decrement("B", 80);
        self.pn_failure_balance = Some(pn_a.merge(&pn_b).value());
    }

    fn sync_if_connected(&mut self) {
        if self.a.is_online() && self.b.is_online() {
            self.sync_all();
        }
    }

    fn sync_all(&mut self) {
        let a_operations = self.a.log.operations.clone();
        let b_operations = self.b.log.operations.clone();
        self.a.ingest_many(b_operations);
        self.b.ingest_many(a_operations);
    }

    fn snapshot(&self) -> FrontendSnapshot {
        FrontendSnapshot {
            clients: vec![self.client_snapshot(&self.a), self.client_snapshot(&self.b)],
            states_equal: self.a.state == self.b.state,
            pn_failure_balance: self.pn_failure_balance,
            messages: self.messages.iter().rev().cloned().collect(),
        }
    }

    fn client_snapshot(&self, client: &LocalClient) -> ClientSnapshot {
        let round = client
            .state
            .round_views
            .get("round-1")
            .map(|round| RoundSnapshot {
                id: round.id.clone(),
                phase: round.phase,
                result: round.result,
            });

        ClientSnapshot {
            replica_id: client.replica_id.clone(),
            online: client.is_online(),
            balance: client.state.player_balance("liam"),
            rights_a: client.state.available_rights("liam", "A"),
            rights_b: client.state.available_rights("liam", "B"),
            operation_count: client.log.operations.len(),
            rejected_count: client.state.rejected_operations.len(),
            round,
            bets: client.state.visible_bets().into_iter().collect(),
        }
    }

    fn trim_messages(&mut self) {
        let excess = self.messages.len().saturating_sub(8);
        if excess > 0 {
            self.messages.drain(0..excess);
        }
    }
}

fn online_label(online: bool) -> &'static str {
    if online { "online" } else { "offline" }
}

#[derive(Debug, Deserialize)]
struct ActionRequest {
    action: String,
    replica: Option<String>,
    amount: Option<u64>,
    bet_kind: Option<String>,
    dice: Option<u8>,
}

#[derive(Debug, Serialize)]
struct FrontendSnapshot {
    clients: Vec<ClientSnapshot>,
    states_equal: bool,
    pn_failure_balance: Option<i64>,
    messages: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ClientSnapshot {
    replica_id: String,
    online: bool,
    balance: i64,
    rights_a: i64,
    rights_b: i64,
    operation_count: usize,
    rejected_count: usize,
    round: Option<RoundSnapshot>,
    bets: Vec<Bet>,
}

#[derive(Debug, Serialize)]
struct RoundSnapshot {
    id: String,
    phase: RoundPhase,
    result: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::{ActionRequest, FrontendDemo};

    #[test]
    fn frontend_demo_applies_offline_bets_and_merges() {
        let mut demo = FrontendDemo::new();
        demo.apply(ActionRequest {
            action: "toggle-online".to_string(),
            replica: Some("B".to_string()),
            amount: None,
            bet_kind: None,
            dice: None,
        });
        demo.apply(ActionRequest {
            action: "bet".to_string(),
            replica: Some("A".to_string()),
            amount: Some(50),
            bet_kind: Some("odd".to_string()),
            dice: None,
        });
        demo.apply(ActionRequest {
            action: "bet".to_string(),
            replica: Some("B".to_string()),
            amount: Some(30),
            bet_kind: Some("even".to_string()),
            dice: None,
        });

        assert!(!demo.snapshot().states_equal);
        demo.apply(ActionRequest {
            action: "toggle-online".to_string(),
            replica: Some("B".to_string()),
            amount: None,
            bet_kind: None,
            dice: None,
        });

        let snapshot = demo.snapshot();
        assert!(snapshot.states_equal);
        assert_eq!(snapshot.clients[0].balance, 20);
        assert_eq!(snapshot.clients[0].bets.len(), 2);
    }
}
