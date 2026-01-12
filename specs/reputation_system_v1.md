# Mostro Reputation & Statistics System Specification (v1)

## 1. Abstract

This document outlines a mechanism to quantify the reliability of Mostro P2P Lightning nodes. By analyzing public Nostr events, we can derive objective metrics (volume, longevity, dispute rates) and subjective metrics (user ratings) to create a "Trust Score". This incentivizes node operators to maintain honest long-term operations rather than executing short-term scams.

## 2. Goals

1.  **Transparency:** Provide users with verifiable data about a Mostro node's history.
2.  **Scam Deterrence:** Increase the economic cost of scamming by making reputation a valuable asset that takes time and volume to build.
3.  **Decentralized Analysis:** Allow any user to run a CLI tool to verify these stats independently.

## 3. Data Sources (Nostr Events)

The system relies primarily on `kind: 38383` (Mostro Protocol) events.

### 3.1 Objective Metrics (Node Performance)

We analyze events published **by the Mostro Node Pubkey**.

| Event Type | Tag Filter | Metric Derived |
| :--- | :--- | :--- |
| **Mostro Instance Status** | `k:38383`, `z=info` | **Days Active:** Calculate from `created_at` timestamp to current date. This event serves as the reference point for when the mostrod started operating. |
| **Completed Order** | `k:38383`, `z=order`, `s=success` | **Volume:** Sum of `amt` (sats). <br> **Count:** Total successful trades. |

> **Note:** Only "success" orders are counted for reputation. "Pending" or "Canceled" orders are ignored to prevent manipulation.

> **Longevity Calculation:** The number of days a mostrod has been active is determined using the "Mostro Instance Status" event (kind 38383, z=info). This event's `created_at` timestamp serves as the reference point for when the instance started. Since this event does not expire, it provides a reliable and permanent indicator of instance age.

> **Implementation Note (v1):** The current CLI tool focuses exclusively on order events and does not track disputes. Dispute tracking may be added in a future version.

#### 3.1.1 Mostro Instance Status Event Structure

The Mostro Instance Status event is used to determine how long a mostrod has been active:

- **Kind:** 38383
- **Tag z:** "info"
- **Key Fields:**
  - `created_at`: Unix timestamp indicating when the mostrod instance started
  - `d`: The Mostro's pubkey (used as identifier)
  - `mostro_version`: Version of the daemon
  - Additional operational parameters (fees, limits, etc.)

This event does not expire and serves as a permanent reference for the instance's start date. For complete event structure documentation, see `docs/protocol/other_events.md`.

### 3.2 Subjective Metrics (User Ratings)

Since a malicious Mostro could censor ratings if they were hosted inside its own protocol events, Users must publish ratings as independent events.

**Proposed Event Structure for Node Reviews:**

*   **Kind:** `1985` (Label) or `1` (Text Note) with specific tags.
*   **Format:**
    ```json
    {
      "kind": 1, 
      "tags": [
        ["p", "<mostro_pubkey>"],     // The node being rated
        ["l", "mostro-review"],       // Label/Namespace
        ["rating", "5"],              // 1-5 Scale
        ["difficulty", "4"],          // Optional: How hard was it?
        ["t", "scam-alert"]           // Optional: If reporting fraud
      ],
      "content": "Fast settlement, good liquidity."
    }
    ```

## 4. The CLI Tool (`mostro-score`)

The first stage of implementation is a Rust CLI tool.

### 4.1 Functional Requirements

1.  **Connection:** Connect to a list of defined Nostr relays.
2.  **Target:** Accept a Mostro Pubkey to analyze (or "scan all" to find known Mostros).
3.  **Ingestion:**
    *   Fetch `kind: 38383` events signed by the target Pubkey:
        *   `z=info` tag for instance status and longevity calculation
        *   `z=order` tag for order events (query-level filtering)
    *   Filter duplicates (using `d` tag as unique Order ID).
4.  **Analysis:**
    *   Compute `Total Volume (sats)`.
    *   Compute `Total Successful Orders`.
    *   Compute `First Seen` (Date) and `Last Seen` (Date).
5.  **Heuristics (Scam Detection):**
    *   *Warning:* If `Average Volume` spikes abnormally in a short time (potential exit scam preparation).
6.  **Output:**
    *   JSON output for integration with other tools/UIs.
    *   Human-readable table for terminal users.

### 4.2 Calculation Logic (Formula Draft)

```rust
// Simplified Trust Score
// days_active is calculated from Mostro Instance Status event (z=info)
let volume_weight = 0.4;
let age_weight = 0.3;
let success_count_weight = 0.3;

// Penalties
let dispute_penalty = dispute_count * 5000; // Heavily penalize disputes

let base_score = (total_sats_volume * volume_weight) 
               + (days_active * age_weight)
               + (success_count * success_count_weight);

let final_score = base_score - dispute_penalty;
```

## 5. Incentive Theory

### The "Cost of Scamming"
For a scammer to be "trusted" enough to steal a large amount (e.g., 1 BTC), they must first build a reputation that attracts that liquidity.
*   If they start with 0 reputation, users will only trade small amounts (e.g., 50k sats).
*   To steal 1 BTC, they need users to open 1 BTC orders.
*   To get users to open 1 BTC orders, the node needs a high "Trust Score".
*   Building that score requires thousands of legitimate trades and months of operation.
*   **Result:** The profit from legitimate operation (fees) over that time becomes greater than the one-time profit of the exit scam.

## 6. Implementation Stages

1.  **Stage 1 (Current):** CLI Tool to aggregate existing `38383` events and display raw stats (Volume, Success Count, Disputes).
2.  **Stage 2:** Implement "User Review" event publishing in Mostro Clients.
3.  **Stage 3:** Integrate `mostro-score` logic into Mostro Clients to show a "Trust Shield" or "Warning" icon next to Mostro nodes in the UI.
