
# 🔄 Solana Week 3: Constant Product Automated Market Maker (AMM)

A decentralized Constant Product Market Maker built on Solana using the Anchor framework. This program enables permissionless liquidity pool creation, liquidity provision, token swaps, split LP/protocol fee routing to a treasury account, and native integer CPMM math without external curve libraries.

---

## ✅ What Was Built & Changed

### 1. Core AMM Instructions (`programs/amm-video/src/instructions/`)

- **`initialize`**: Creates a new liquidity pool `Config` PDA, initializes the LP token mint (`mint_lp`), sets up token vaults (`vault_x` and `vault_y`), and configures swap fees alongside a dedicated `treasury` account.
- **`deposit`**: Allows Liquidity Providers (LPs) to deposit Token X and Token Y into the pool vaults in proportion to current reserves and mints LP tokens to their wallet (`user_lp`). Includes slippage checks (`max_x`, `max_y`).
- **`withdraw`**: Allows LPs to burn their LP tokens and redeem their proportional share of Token X and Token Y from the vaults, protected by minimum output bounds (`min_x`, `min_y`).
- **`swap`**: Executes bidirectional token swaps (`Token X -> Token Y` or `Token Y -> Token X`) using the constant-product curve while enforcing minimum output slippage limits (`min_amount_out`).

---

### 2. Fee Split & Treasury Account Routing (`state.rs`, `initialize.rs`, `swap.rs`)

- **Updated `Config` State**: Added `treasury: Pubkey` and `protocol_fee: u16` (measured in basis points) alongside the existing LP `fee: u16`.

- **Fee Split Logic on Swaps**:
  - **Protocol Fee (`protocol_fee` bps)**: Deducted directly from the input amount and transferred via SPL Token CPI straight into the treasury's Associated Token Account (`treasury_x` or `treasury_y`).
  - **LP Swap Fee (`fee` bps)**: Retained inside the pool vaults (`vault_x` / `vault_y`), increasing the constant product `k` so liquidity providers earn yield automatically on every trade.

- **Safety Validation**: Added a `require!(fee + protocol_fee <= 10_000)` check during initialization so total fees can never exceed 100%.

---

### 3. Custom CPMM Math Without External Libraries (`deposit.rs`, `withdraw.rs`, `swap.rs`, `error.rs`)

- **Removed External Dependency**: Stripped out the `constant-product-curve` crate from `Cargo.toml` and all instruction/error modules.

- **Native `u128` Checked Arithmetic**: Implemented pure on-chain constant product formulas using overflow-safe `checked_mul`, `checked_div`, `checked_add`, and `checked_sub`:
  - **Swap Output Formula**:

    \[
    \Delta y = \frac{y \cdot \Delta x_{\text{net}}}{x + \Delta x_{\text{net}}}
    \]

    where \(\Delta x_{\text{net}}\) is the input amount after subtracting both `protocol_fee` and `fee`.

  - **Proportional Deposit & Withdraw Math**: Calculated exact token ratios using `(vault_balance * lp_amount) / lp_supply` in `u128` before casting safely back to `u64`.

---

### 4. LiteSVM Integration Tests (`programs/amm-video/tests/`)

- **Updated Instruction Builders (`tests/ix_handlers/init.rs` & `swap.rs`)**: Wired `treasury`, `protocol_fee` (`10` bps), and treasury ATAs (`treasury_x`, `treasury_y`) into the test transactions.

- **Full Instruction Coverage (`tests/tests.rs`)**:
  - `test_initialize`: Verifies pool configuration and vault initialization.
  - `test_deposit`: Verifies initial liquidity deposit and LP token minting.
  - `test_withdraw`: Verifies LP token burning and proportional asset redemption.
  - `test_swap`: Executes a swap and asserts that `treasury_x` receives the exact `10` bps protocol fee (`10,000` base units on a `10,000,000` swap).

---

## 🛡️ Extension: Mitigating Downtime in DeFi Applications

To keep a Solana AMM resilient during network congestion, RPC outages, or market volatility, the following safeguards are used:

1. **On-Chain Circuit Breakers (`locked` Flag & `authority`)**
   - The `Config` account stores a `locked: bool` flag controlled by the pool `authority`. If an extreme price anomaly, oracle issue, or vulnerability is detected, the authority can lock the pool immediately to halt swaps and deposits while keeping user funds safe.

2. **Multi-RPC Failover & Load Balancing**
   - Instead of relying on a single public RPC node, frontend clients route transactions across multiple dedicated RPC providers (e.g., Helius, Triton, QuickNode) with automatic health checks and fallback switching if one endpoint degrades.

3. **Dynamic Priority Fees & Compute Budget Optimization**
   - During high-traffic periods on Solana, clients attach `ComputeBudgetProgram.setComputeUnitPrice` instructions based on recent fee percentiles so swap and liquidity transactions are prioritized by block leaders rather than dropped.

4. **Strict Slippage & Stale-State Protection**
   - Every `deposit`, `withdraw`, and `swap` instruction enforces user-defined bounds (`max_x`, `max_y`, `min_x`, `min_y`, `min_amount_out`). If network delays cause a transaction to land after the pool price has shifted, the program aborts cleanly instead of executing at an unfavorable rate.

5. **Stateless Execution & PDA Determinism**
   - All pool vaults and configurations are derived deterministically via Program Derived Addresses (`[b"config", seed]` and `[b"lp", config]`), eliminating single-server bottlenecks and allowing any client or aggregator (such as Jupiter) to reconstruct accounts and interact with the pool directly on-chain.

---

## 📂 Project Structure

*Only changed part*

```text
amm_q2_26/
├── programs/
│   └── amm-video/
│       ├── src/
│       │   ├── instructions/
│       │   │   ├── initialize.rs   # Pool & treasury setup
│       │   │   ├── deposit.rs      # Custom CPMM proportional deposit
│       │   │   ├── withdraw.rs     # Custom CPMM proportional withdrawal
│       │   │   └── swap.rs         # Custom x*y=k swap + treasury fee split
│       │   ├── state.rs            # Config account with treasury & protocol_fee
│       │   ├── error.rs            # Custom AMM error codes
│       │   └── lib.rs              # Program entrypoints
│       ├── tests/
│       │   ├── ix_handlers/        # LiteSVM instruction builders
│       │   └── tests.rs            # End-to-end LiteSVM test suite
│       └── Cargo.toml
└── Anchor.toml
```

---

## 🚀 How to Build & Run Tests

Run the following commands from the project root:

```bash
# Synchronize program IDs
anchor keys sync

# Build the Anchor program
anchor build

# Run the LiteSVM test suite
cargo test --manifest-path programs/amm-video/Cargo.toml
```









