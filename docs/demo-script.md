# BetSync Demo Script

Use this script for a repeatable presentation demo.

## Demo 1: Double-Spend Problem and Fix

```bash
cargo run -- simulate double-spend
```

Expected talking point:

```text
PN-counter only failure:
100 - 80 - 80 = -60

Bounded counter:
A has 60 rights
B has 40 rights
80-chip bets are rejected
50 + 30 is accepted
merged balance remains non-negative
```

## Demo 2: Offline Merge

```bash
cargo run -- simulate offline-merge
```

Expected result:

```text
A and B both contain both bets
A and B have the same final balance
A and B states are equal
```

## Demo 3: Live Sync

Terminal 1:

```bash
cargo run -- server --port 8080
```

Terminal 2:

```bash
cargo run -- client --replica-id A --server 127.0.0.1:8080
```

Terminal 3:

```bash
cargo run -- client --replica-id B --server 127.0.0.1:8080
```

Client A:

```text
create-table BetSync
join-split liam Liam A:60 B:40
start-round round-1
```

Client B:

```text
state
bet even 30 liam round-1
```

Client A:

```text
bet odd 50 liam round-1
state
rights liam
```

## Demo 4: Offline Queue and Reconnect

Client B:

```text
offline
bet even 30 liam round-1
log
```

Client A:

```text
bet odd 50 liam round-1
state
```

Client B:

```text
online
state
```

Both clients should converge after the queued operations and missing server operations are exchanged.

## Demo 5: Duplicate Message Safety

On either client:

```text
log
resend <op_id>
state
```

The operation log may receive the duplicate over the network, but client state does not double-apply it because `seen_operations` deduplicates by `op_id`.
