# docs/05 — Auction Economics (pre_nav anchored)

> Status: **frozen for P0**. これは `axis-auction` POC の唯一の経済式定義です。
> `programs/axis-auction/src/math.rs` はこの doc を実装し、unit test で pin します。
> production DTF Core の会計ではありません（POC の mock economics）。

---

## 0. 単位・スケール

| 概念 | 単位 / scale | 定数 |
|---|---|---|
| **SOL** | lamports | — （**transaction fee / priority fee 専用**。bid/payment には使わない） |
| **mock USDC** | base units, **6 decimals**（`1 USDC = 1_000_000`） | `USDC_DECIMALS = 6` |
| **mock DTF** | base units, **6 decimals** | `DTF_DECIMALS = 6` |
| **price (NAV / pool price)** | USDC per DTF, **1e6 scale** | `PRICE_SCALE = 1_000_000` |
| **bps** | **1e4 scale** | `BPS_SCALE = 10_000` |

> **Cluster 前提:** Testnet に deploy / tx 実行する場合、**Devnet SOL は使えない。Testnet SOL が必要**。
> Devnet と Testnet は別 cluster。SOL は fee 用途のみ。bid/payment は mock USDC SPL mint を使う。

金額系は `u64`、差分・recapture など符号付きは `i64`。計算は内部 `i128/u128` で行い、
`checked_*` で overflow を防ぐ。丸めは floor。

---

## 1. 経済モデル（pre_nav を起点にする）

DTF の現在価格 `pre_nav` が真の `target_nav` から乖離している。これを是正（settlement /
correction）するには `batch_size` 分の DTF を clearing する必要がある。

- **起点 (pre_nav):** 是正前の価格。ここからの「ズレの大きさ」がそもそもの問題のサイズ。
- **目標 (target_nav):** 是正後にあるべき価格。
- **winner の到達点 (mock_pool_price):** winner が mock pool で実際に約定できる価格。
- **auction なし:** protocol が naive に是正 → `expected_cost_without_auction`（ベースライン。
  既定値は「起点の gap 全額を被る」= `starting_gap_value`。注入で下げることも可能）。
- **auction あり:** 専門家 (winner) が `mock_pool_price` で効率的に是正し、権利対価
  `winner_bid_amount` を支払う。

`pre_nav` は次の 3 箇所で本質的に使われる:
1. `starting_gap_value`（そもそもの是正対象サイズ）
2. `improvement_bps`（起点 gap をどれだけ閉じたか）
3. `expected_cost_without_auction` の既定値

---

## 2. 入力値

| 名前 | 意味 | 単位 | 由来 |
|---|---|---|---|
| `batch_size` | clearing 対象 DTF 数量 | DTF base(6) | market/round |
| `pre_nav` | 是正前 NAV/price（起点） | price(1e6) | market（round に snapshot） |
| `target_nav` | 目標 NAV/price | price(1e6) | market |
| `mock_pool_price` | winner が約定できる mock 価格 | price(1e6) | market |
| `expected_cost_without_auction` | auction なしベースライン cost | USDC(6) | market（既定 = starting_gap_value） |
| `winner_bid_amount` | 落札額（権利対価） | USDC(6) | round.highest_bid |
| `protocol_fee_bps` | 落札収益の protocol 取り分 | bps(1e4) | config |
| `min_settlement_out` | settlement 最低産出 | USDC(6) | market |
| `min_improvement_bps` | settlement 最低改善率 | bps(1e4) | market/config |

---

## 3. 計算式（決定論。`math.rs` に実装し test で pin）

内部は `u128/i128`。表記の `abs_diff(a,b) = max(a,b) - min(a,b)`。

```text
# --- pre_nav 起点のサイズ ---
starting_gap_per_unit  = abs_diff(target_nav, pre_nav)                         # price(1e6)
starting_gap_value     = batch_size * starting_gap_per_unit / PRICE_SCALE      # USDC(6)

# --- winner の到達コスト ---
residual_gap_per_unit  = abs_diff(target_nav, mock_pool_price)                 # price(1e6)
settlement_cost        = batch_size * residual_gap_per_unit / PRICE_SCALE      # USDC(6)
settlement_out         = batch_size * mock_pool_price / PRICE_SCALE            # USDC(6)

# --- 効率改善（pre_nav 起点でどれだけ gap を閉じたか） ---
gap_closed_value       = (i128) starting_gap_value - (i128) settlement_cost    # USDC(6, i128)

# --- auction なし比較（ベースライン比） ---
gross_cost_reduction   = (i128) expected_cost_without_auction
                         - (i128) settlement_cost                              # USDC(6, i128)

# --- 落札収益と分配 ---
auction_revenue        = winner_bid_amount                                     # USDC(6)
protocol_revenue       = auction_revenue * protocol_fee_bps / BPS_SCALE        # USDC(6)
creator_revenue        = auction_revenue - protocol_revenue                    # USDC(6)

# --- 取り戻した総価値 / 当事者別純便益 ---
total_value_recaptured = gross_cost_reduction + (i128) auction_revenue         # USDC(6, i128)
net_protocol_benefit   = gross_cost_reduction + (i128) protocol_revenue        # USDC(6, i128)
net_creator_benefit    = (i128) creator_revenue                               # USDC(6, i128)

# --- 改善率（pre_nav 起点 gap に対する閉じ率） ---
improvement_bps        = if starting_gap_value == 0 { 0 }
                         else { gap_closed_value * BPS_SCALE / (i128) starting_gap_value }
```

### 検証条件（settlement 成立に必須。P0 では math 層で関数提供、ix 層は T0-3 以降）
```text
require( settlement_out  >= min_settlement_out )         # 不足: MinOutNotMet
require( improvement_bps >= (i128) min_improvement_bps ) # 不足: MinImprovementNotMet
```

---

## 4. 数値の意味・単位・保存先・event field

| 数値 | 意味 | 単位/型 | 保存先(account) | event field |
|---|---|---|---|---|
| `pre_nav` | 起点 NAV | price(1e6) u64 | market / round.nav_snapshot / receipt | MarketCreated / MockSettlementExecuted |
| `target_nav` | 目標 NAV | price(1e6) u64 | market / receipt | MockSettlementExecuted |
| `mock_pool_price` | winner 約定価格 | price(1e6) u64 | market / receipt | MockSettlementExecuted |
| `batch_size` | clearing 数量 | DTF(6) u64 | market / receipt | MarketCreated |
| `starting_gap_value` | 起点 gap の USDC 価値 | USDC(6) u64 | receipt | MockSettlementExecuted |
| `expected_cost_without_auction` | auction なし baseline cost | USDC(6) u64 | market / receipt | MockSettlementExecuted |
| `settlement_out` | settlement 産出 USDC | USDC(6) u64 | receipt | MockSettlementExecuted |
| `settlement_cost` | winner 実コスト | USDC(6) u64 | receipt | MockSettlementExecuted |
| `winner_bid_amount` | 落札額 | USDC(6) u64 | receipt / winner_auth | MockSettlementExecuted / AuctionClosed |
| `gap_closed_value` | 起点比で閉じた gap | USDC(6) i64 | receipt | MockSettlementExecuted |
| `gross_cost_reduction` | baseline − 実コスト | USDC(6) i64 | receipt | MockSettlementExecuted |
| `auction_revenue` | 落札収益(=bid) | USDC(6) u64 | receipt | MockSettlementExecuted |
| `total_value_recaptured` | 効率改善+収益 | USDC(6) i64 | receipt | MockSettlementExecuted |
| `protocol_revenue` | 収益の protocol 分 | USDC(6) u64 | receipt / protocol_vault.total_in | MockSettlementExecuted / AuctionPaymentRecorded |
| `creator_revenue` | 収益の creator 分 | USDC(6) u64 | receipt / creator_vault.total_in | MockSettlementExecuted / AuctionPaymentRecorded |
| `net_protocol_benefit` | 効率改善+protocol fee | USDC(6) i64 | receipt | MockSettlementExecuted |
| `net_creator_benefit` | creator 純便益 | USDC(6) i64 | receipt | MockSettlementExecuted |
| `improvement_bps` | 起点 gap の閉じ率 | bps(1e4) i64 | receipt | MockSettlementExecuted |
| `min_settlement_out` | 最低産出制約 | USDC(6) u64 | market | MarketCreated |
| `min_improvement_bps` | 最低改善率制約 | bps(1e4) u16 | market/config | MarketCreated / ConfigInitialized |

> i128 で計算した値を account(i64/u64) に保存する際は downcast を `checked` で行い、
> 溢れたら error（T0-3 以降の ix 層で実施）。

---

## 5. 不変条件（invariant / unit test で assert）

1. `creator_revenue + protocol_revenue == auction_revenue`
2. `auction_revenue == winner_bid_amount`
3. `total_value_recaptured == gross_cost_reduction + auction_revenue`
4. `net_protocol_benefit == gross_cost_reduction + protocol_revenue`
5. `net_creator_benefit == creator_revenue`
6. `gap_closed_value == starting_gap_value - settlement_cost`
7. `starting_gap_value == 0` のとき `improvement_bps == 0`
8. すべて `checked_*`：overflow しない
9. **reserve に一切触れない**（reserve は account にもコードにも存在しない）

---

## 6. 参考 worked example（test に pin される値）

| 入力 | 値 |
|---|---|
| batch_size | 1_000_000_000 （1000 DTF） |
| pre_nav | 1_000_000 （1.00） |
| target_nav | 1_050_000 （1.05） |
| mock_pool_price | 1_040_000 （1.04） |
| expected_cost_without_auction | 50_000_000 （50 USDC） |
| winner_bid_amount | 5_000_000 （5 USDC） |
| protocol_fee_bps | 2000 （20%） |

| 出力 | 値 |
|---|---|
| starting_gap_value | 50_000_000 |
| settlement_cost | 10_000_000 |
| settlement_out | 1_040_000_000 |
| gap_closed_value | 40_000_000 |
| gross_cost_reduction | 40_000_000 |
| auction_revenue | 5_000_000 |
| total_value_recaptured | 45_000_000 |
| protocol_revenue | 1_000_000 |
| creator_revenue | 4_000_000 |
| net_protocol_benefit | 41_000_000 |
| net_creator_benefit | 4_000_000 |
| improvement_bps | 8000 （80%） |

---

## 7. P0 record-only → P1 SPL transfer（payment）

- **P0:** `claim_or_record_auction_payment` は record-only。`protocol_revenue` /
  `creator_revenue` を vault の `total_in` に積算し event を出すだけ（token 移動なし）。
- **P1:** 同 instruction に mock USDC SPL `transfer` CPI を追加し、winner の USDC ATA から
  protocol / creator vault の ATA(PDA authority) へ実分配。account 構造は P0 から前方互換に保つ。
- どちらの phase でも **reserve account は存在しない**（accounting cross を構造的に不可能にする）。
