const replicasEl = document.querySelector("#replicas");
const messagesEl = document.querySelector("#messages");
const statusEl = document.querySelector("#convergenceStatus");
const roundPhaseEl = document.querySelector("#roundPhase");
const diceFaceEl = document.querySelector("#diceFace");
const pnValueEl = document.querySelector("#pnValue");
const pnBalanceEl = document.querySelector("#pnBalance");
const rightsTotalEl = document.querySelector("#rightsTotal");
const rightsMeterEl = document.querySelector("#rightsMeter");
const betsTableEl = document.querySelector("#betsTable");
const betCountEl = document.querySelector("#betCount");
const logCountEl = document.querySelector("#logCount");

let state = null;

document.querySelector("#resetBtn").addEventListener("click", () => act({ action: "reset" }));
document.querySelector("#syncBtn").addEventListener("click", () => act({ action: "sync" }));
document.querySelector("#pnBtn").addEventListener("click", () => act({ action: "pn-failure" }));
document.querySelector("#closeBtn").addEventListener("click", () => act({ action: "close-round" }));
document.querySelector("#resolveBtn").addEventListener("click", () => {
  const dice = Number(document.querySelector("#diceSelect").value);
  act({ action: "resolve-round", dice });
});

async function loadState() {
  const response = await fetch("/api/state");
  state = await response.json();
  render();
}

async function act(payload) {
  const response = await fetch("/api/action", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  state = await response.json();
  render();
}

function render() {
  renderSummary();
  renderReplicas();
  renderBets();
  renderMessages();
}

function renderSummary() {
  const phase = state.clients[0]?.round?.phase ?? "Unknown";
  const result = state.clients[0]?.round?.result ?? 1;
  const balance = state.clients[0]?.balance ?? 0;
  const rightsA = state.clients[0]?.rights_a ?? 0;
  const rightsB = state.clients[0]?.rights_b ?? 0;
  const remaining = rightsA + rightsB;

  statusEl.textContent = state.states_equal
    ? "Replicas have converged"
    : "Replicas currently differ";
  statusEl.style.color = state.states_equal ? "var(--green)" : "var(--amber)";

  roundPhaseEl.textContent = phase;
  roundPhaseEl.className = `pill ${phase === "Resolved" ? "muted" : ""}`;
  diceFaceEl.className = `dice dice-${result || 1}`;

  if (state.pn_failure_balance === null || state.pn_failure_balance === undefined) {
    pnValueEl.textContent = "PN: not run";
    pnValueEl.className = "pill muted";
    pnBalanceEl.textContent = "-";
  } else {
    pnValueEl.textContent = "PN: negative";
    pnValueEl.className = "pill offline";
    pnBalanceEl.textContent = state.pn_failure_balance;
  }

  rightsTotalEl.textContent = `${remaining} / balance ${balance}`;
  rightsMeterEl.style.width = `${Math.max(0, Math.min(100, remaining))}%`;
}

function renderReplicas() {
  replicasEl.innerHTML = state.clients.map((client) => `
    <article class="panel replica">
      <div class="panel-head">
        <h2>Replica ${escapeHtml(client.replica_id)}</h2>
        <span class="pill ${client.online ? "" : "offline"}">
          ${client.online ? "online" : "offline"}
        </span>
      </div>

      <div class="stats">
        <div class="stat">
          <span>Balance</span>
          <strong>${client.balance}</strong>
        </div>
        <div class="stat">
          <span>A rights</span>
          <strong>${client.rights_a}</strong>
        </div>
        <div class="stat">
          <span>B rights</span>
          <strong>${client.rights_b}</strong>
        </div>
      </div>

      <div class="inline-controls">
        <button type="button" data-action="toggle-online" data-replica="${client.replica_id}">
          ${client.online ? "Go offline" : "Go online"}
        </button>
        <span class="pill muted">${client.operation_count} ops</span>
        <span class="pill ${client.rejected_count ? "warn" : "muted"}">${client.rejected_count} rejected</span>
      </div>

      <div class="bet-controls">
        <select data-field="kind" data-replica="${client.replica_id}" aria-label="Bet kind">
          <option value="odd">Odd</option>
          <option value="even">Even</option>
          <option value="high">High</option>
          <option value="low">Low</option>
          <option value="exact:6">Exact 6</option>
        </select>
        <input class="amount-input" data-field="amount" data-replica="${client.replica_id}" type="number" min="1" max="100" value="${client.replica_id === "A" ? 50 : 30}" aria-label="Bet amount" />
        <button class="primary" type="button" data-action="bet" data-replica="${client.replica_id}">Bet</button>
      </div>
    </article>
  `).join("");

  replicasEl.querySelectorAll("[data-action='toggle-online']").forEach((button) => {
    button.addEventListener("click", () => act({
      action: "toggle-online",
      replica: button.dataset.replica,
    }));
  });

  replicasEl.querySelectorAll("[data-action='bet']").forEach((button) => {
    button.addEventListener("click", () => {
      const replica = button.dataset.replica;
      const kind = replicasEl.querySelector(`[data-field='kind'][data-replica='${replica}']`).value;
      const amount = Number(replicasEl.querySelector(`[data-field='amount'][data-replica='${replica}']`).value);
      act({ action: "bet", replica, amount, bet_kind: kind });
    });
  });
}

function renderBets() {
  const bets = state.clients[0]?.bets ?? [];
  betCountEl.textContent = `${bets.length}`;
  logCountEl.textContent = `${state.clients.reduce((sum, client) => sum + client.operation_count, 0)} ops`;

  if (!bets.length) {
    betsTableEl.innerHTML = "<p class=\"status-line\">No bets</p>";
    return;
  }

  betsTableEl.innerHTML = `
    <table>
      <thead>
        <tr>
          <th>Replica</th>
          <th>Player</th>
          <th>Kind</th>
          <th>Amount</th>
          <th>Round</th>
        </tr>
      </thead>
      <tbody>
        ${bets.map((bet) => `
          <tr>
            <td>${escapeHtml(bet.origin_replica)}</td>
            <td>${escapeHtml(bet.player_id)}</td>
            <td>${formatBetKind(bet.kind)}</td>
            <td>${bet.amount}</td>
            <td>${escapeHtml(bet.round_id)}</td>
          </tr>
        `).join("")}
      </tbody>
    </table>
  `;
}

function renderMessages() {
  messagesEl.innerHTML = state.messages
    .map((message) => `<li>${escapeHtml(message)}</li>`)
    .join("");
}

function formatBetKind(kind) {
  if (typeof kind === "string") return escapeHtml(kind);
  if (kind && typeof kind === "object" && "Exact" in kind) return `Exact ${kind.Exact}`;
  return escapeHtml(JSON.stringify(kind));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

loadState().catch((error) => {
  statusEl.textContent = `Frontend error: ${error.message}`;
  statusEl.style.color = "var(--red)";
});
