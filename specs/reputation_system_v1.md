# Mostro Reputation & Statistics System Specification (v1.1)

## 1. Abstract

This document specifies a mechanism to quantify the reliability and operational health of Mostro P2P Lightning nodes ("mostrod").

By analyzing public Nostr events, the system derives **objective historical metrics** and **objective activity (liveness) metrics**, enabling users to make informed decisions before trading.

The specification is designed to support:
- Rust CLI tools
- Terminal UIs (TUI)
- Web dashboards

The protocol defines **what data must be computed**, not how it must be rendered.

---

## 2. Goals

1. **Transparency:** Provide verifiable insight into a Mostro node’s past and present behavior.
2. **User Safety:** Clearly expose inactivity, degradation, or irregular activity patterns.
3. **Scam Deterrence:** Make exit scams economically irrational by surfacing continuity signals.
4. **Presentation-Agnostic:** Allow multiple frontends (CLI, TUI, Web) to consume the same data model.

---

## 3. Data Sources (Nostr Events)

All metrics are derived exclusively from public Nostr events.

### 3.1 Mostro Order Events

- **Kind:** `38383`
- **Required Tags:**
  - `z=order`
  - `y=mostro`
  - `s=success`
- **Publisher:** Mostro node pubkey

Only successful orders are considered for reputation and activity metrics.

---

### 3.2 Development Fee Payment Events (Longevity Anchor)

Development fee payment events are used to anchor real trading activity.

- **Kind:** `8383`
- **Required Tags:**
  - `z=dev-fee-payment`
  - `y=mostro`
- **Publisher:** Mostro node pubkey

> **Longevity Rule:**  
> The oldest development fee payment event marks the start of actual trading activity.

---

## 4. Canonical Data Model (Internal Representation)

Implementations MUST compute and expose the following logical data groups.  
How they are rendered (tables, panels, charts) is frontend-specific.

---

## 4.1 Historical Reputation Metrics

These metrics describe the accumulated track record of a Mostro node.

### 4.1.1 Longevity

- **first_seen_at**  
  Timestamp of the earliest dev-fee-payment event.
- **days_active**  
  Number of days since first_seen_at.

---

### 4.1.2 Cumulative Trade Performance

Derived from successful order events.

- **total_successful_trades**
- **total_volume_sats**

---

### 4.1.3 Trade Amount Statistics (Lifetime)

Computed only from successful trades.

- **min_trade_sats**
- **max_trade_sats**
- **mean_trade_sats**
- **median_trade_sats**

#### Definitions and Interpretation

- **mean_trade_sats**  
  The arithmetic average of all successful trade amounts, calculated as the total traded volume divided by the number of successful trades.

  This value represents the *average volume per trade* over the node’s lifetime.  
  It **MAY be influenced by large outliers** (e.g., a few very large trades).

- **median_trade_sats**  
  The middle value of all successful trade amounts when ordered from smallest to largest.

  This value represents the *typical trade size* and is **robust against outliers**.

#### Important Distinction

- The **mean** answers:  
  *“On average, how much volume does each trade represent?”*
- The **median** answers:  
  *“What trade size is most typical for this node?”*

Because exit scams and reputation manipulation often rely on a small number of large trades, **median_trade_sats MUST be treated as the primary reference for typical trade size**, while mean_trade_sats SHOULD be considered a secondary, contextual metric.

> **Design Requirement:**  
> Median MUST be computed and exposed, and MUST NOT be replaced by the mean in user-facing risk assessments.

---

## 4.2 Activity & Liveness Metrics (Critical)

These metrics represent **current operational health** and MUST be treated as first-class signals.

---

### 4.2.1 Last Successful Trade

- **last_successful_trade_at**
- **days_since_last_trade**

> This metric MUST be easy to surface prominently in any UI.

---

### 4.2.2 Recent Activity Windows

Successful trades MUST be counted in rolling windows:

- **successful_trades_last_7d**
- **successful_trades_last_30d**
- **successful_trades_last_90d**

> **Design Principle:**  
> Rolling windows are preferred over averages to preserve temporal meaning.

---

### 4.2.3 Activity Consistency (Optional but Recommended)

Used to detect bursty or irregular behavior.

- **active_days_last_30d**  
  Distinct days with ≥1 successful trade.
- **max_consecutive_inactive_days_last_30d**

These values allow frontends to express continuity visually.

---

## 5. Derived Indicators (Presentation-Level, Non-Normative)

Implementations MAY derive higher-level indicators from the canonical data model, such as:

- Activity status labels (e.g., Active / Low Activity / Inactive)
- Suggested safe trade size
- Visual warnings or highlights

> **Important:**  
> Derived indicators MUST NOT replace raw metrics and MUST remain interpretable.

---

## 6. CLI / TUI Tooling Guidelines

### 6.1 CLI Output Expectations

The CLI tool SHOULD present data in a human-readable, structured format, such as:

- Sectioned summaries
- Tables
- Highlighted warnings
- Relative time expressions (e.g., "last trade: 2 days ago")

The CLI MUST NOT rely on raw JSON output as its primary interface.

---

### 6.2 TUI / Dashboard Compatibility

The internal data model SHOULD:

- Be cleanly separable from rendering logic
- Allow multiple visual representations:
  - Tables
  - Time-based charts
  - Status badges
- Support incremental updates (future live dashboards)

---

## 7. Data Integrity Rules

- Orders MUST be deduplicated by order ID (`d` tag)
- Only final successful order states count
- Events MUST be ordered by `created_at`
- Malformed or incomplete events MUST be ignored safely

---

## 8. Design Principles

- Permissionless
- Trustless
- Presentation-agnostic
- Temporal signals are first-class
- No single opaque score required

---

## 9. Non-Goals

This specification explicitly excludes:

- JSON as a mandatory output format
- Subjective ratings
- Scam reports
- Dispute resolution logic
- UI or layout definitions

These are addressed in later stages.

---

## 10. Incentive Alignment

A Mostro operator maximizes long-term profit by:

- Maintaining continuous activity
- Preserving visible operational health
- Avoiding inactivity gaps that raise user risk perception

Nodes that go inactive or behave irregularly become visibly riskier before large trades occur.
