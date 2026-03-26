# SolTrace

**A general-purpose Solana blockchain indexer built in Rust.**

SolTrace tracks on-chain events (deposits, withdrawals, token transfers, and account state changes) in real time and provides a queryable REST API, webhook system, and live dashboard. Built for teams that need custom indexing infrastructure without depending on rate-limited, expensive third-party APIs.

---

## Feature Highlights

- **Real-time ingestion** via Solana RPC WebSocket subscriptions with automatic reconnection and exponential backoff
- **CPI-aware decoding** that processes both outer and inner (CPI) instructions, capturing the ~60-70% of real deposits that happen via CPI (e.g., Jupiter swap outputs)
- **Historical backfill engine** with rate limiting, slot-based checkpointing, and resumable jobs
- **SPL Token + native SOL** support, decoding System Program transfers and SPL Token `Transfer`/`TransferChecked` instructions (Token Program and Token-2022)
- **Deposit/withdrawal classification** that labels each transfer based on whether the watched wallet is the source or destination owner
- **Dynamic Anchor IDL decoding** lets you upload any program's IDL via API to enable automatic instruction parsing
- **Webhook system** to register HTTP callbacks with filters (wallet, direction, mint, minimum amount) for event-driven integrations
- **Filter DSL**, a purpose-built query language: `TRACK transfers WHERE direction = "deposit" AND amount > 1_000_000_000`
- **JWT authentication** with username/password registration and Google OAuth sign-in
- **Per-user wallet scoping** so each user manages their own set of watched wallets
- **Live dashboard** built with Next.js featuring stats cards, transfer charts, transfer history table, dark/light theme, and sidebar navigation
- **Redis caching** for slot cursor tracking, account owner lookup cache, and stream-based ingestion buffering
- **Idempotent writes** using `UNIQUE(signature, instruction_idx)` constraint to prevent duplicate entries on replays
- **Production-ready containerization** with multi-stage Dockerfile and Docker Compose for one-command local deployment

---

## Architecture

```
                          +-------------------+
                          |   Solana Devnet   |
                          |   (RPC + WSS)     |
                          +--------+----------+
                                   |
                    +--------------+--------------+
                    |                             |
            Live (WebSocket)              Backfill (RPC)
          logsSubscribe +             getSignaturesForAddress
          getTransaction              + getTransaction
                    |                             |
                    +-------------+---------------+
                                  |
                          +-------v--------+
                          |   Ingestion    |
                          | (rpc_listener, |
                          |  backfill)     |
                          +-------+--------+
                                  |
                          +-------v--------+
                          |    Decoder     |
                          | system_program |
                          | token_program  |
                          | idl_decoder    |
                          +-------+--------+
                                  |
                          +-------v--------+
                          |   Classifier   |
                          | deposit vs     |
                          | withdrawal     |
                          +-------+--------+
                                  |
                   +--------------+--------------+
                   |                             |
           +-------v--------+           +-------v--------+
           |   PostgreSQL   |           |     Redis      |
           | transfers,     |           | slot cursor,   |
           | wallets, IDLs, |           | account owner  |
           | webhooks, users|           | cache, streams |
           +-------+--------+           +----------------+
                   |
          +--------+---------+
          |                  |
   +------v------+    +-----v-------+
   |  Axum API   |    |  Webhooks   |
   | REST + Auth |    | HTTP push   |
   +------+------+    +-------------+
          |
   +------v------+
   |  Dashboard  |
   | Next.js +   |
   | Recharts    |
   +-------------+
```

---

## Tech Stack

| Component       | Technology                                              |
|-----------------|---------------------------------------------------------|
| Language        | Rust (2021 edition)                                     |
| Web Framework   | Axum 0.8 + Tower middleware                             |
| Database        | PostgreSQL 16 (via sqlx 0.8)                            |
| Cache / Queue   | Redis 7 (streams, cache, slot cursor)                   |
| Solana SDK      | solana-client 2.1, solana-sdk 2.1, solana-transaction-status 2.1 |
| Authentication  | JWT (jsonwebtoken) + Argon2 password hashing + Google OAuth |
| Dashboard       | Next.js 15, React 19, Recharts, TypeScript              |
| Containerization| Docker (multi-stage build) + Docker Compose             |
| Logging         | tracing + tracing-subscriber with env-filter             |
| Config          | config crate (TOML + environment variable overrides)    |

---

## Project Structure

```
soltrace/
├── Cargo.toml                     # Workspace root
├── Cargo.lock
├── Dockerfile                     # Multi-stage production build
├── docker-compose.yml             # PostgreSQL 16 + Redis 7
├── config/
│   └── default.toml               # Default configuration
├── migrations/
│   ├── 001_initial_schema.sql     # Core tables: wallets, token_accounts, transfers
│   ├── 002_phase2_backfill_webhooks_idls.sql
│   ├── 003_users_auth.sql         # Users table, JWT auth
│   ├── 004_google_oauth.sql       # Google OAuth columns
│   └── 005_wallet_user_scope.sql  # Per-user wallet scoping
├── crates/
│   ├── ingestion/                 # WebSocket listener, backfill engine
│   │   └── src/
│   │       ├── rpc_listener.rs    # logsSubscribe with reconnect + backoff
│   │       └── backfill.rs        # Historical indexing with checkpointing
│   ├── decoder/                   # Transaction parsing + classification
│   │   └── src/
│   │       ├── system_program.rs  # Native SOL transfer decoding
│   │       ├── token_program.rs   # SPL Token Transfer/TransferChecked
│   │       ├── classifier.rs      # Deposit vs withdrawal classification
│   │       ├── account_mapper.rs  # Wallet -> token account mapping
│   │       └── idl_decoder.rs     # Dynamic Anchor IDL parsing
│   ├── storage/                   # Database + cache operations
│   │   └── src/
│   │       ├── postgres.rs        # sqlx queries, upserts, dynamic filters
│   │       └── redis_cache.rs     # Slot cursor, owner cache, streams
│   ├── api/                       # REST API layer
│   │   └── src/
│   │       ├── routes.rs          # Axum router definition
│   │       ├── handlers.rs        # Request handlers
│   │       └── auth.rs            # JWT + Google OAuth authentication
│   └── filter_dsl/                # Custom query language
│       └── src/
│           ├── lexer.rs           # Tokenizer
│           ├── parser.rs          # AST construction
│           └── evaluator.rs       # Runtime evaluation
├── bin/
│   └── soltrace/
│       └── src/
│           └── main.rs            # CLI entry point, wires all crates
└── dashboard/                     # Next.js frontend
    ├── package.json
    ├── app/
    │   └── page.tsx               # Overview page
    └── components/
        ├── Shell.tsx              # Layout shell (sidebar + topbar)
        ├── Sidebar.tsx
        ├── Topbar.tsx
        ├── StatsCards.tsx
        ├── TransferChart.tsx
        ├── TransferTable.tsx
        └── WelcomeBanner.tsx
```

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [Node.js](https://nodejs.org/) 18+ (for the dashboard)

### 1. Clone the repository

```bash
git clone https://github.com/your-username/soltrace.git
cd soltrace
```

### 2. Start PostgreSQL and Redis

```bash
docker compose up -d
```

This starts PostgreSQL 16 on port 5432 and Redis 7 on port 6379. Migrations in `migrations/` are applied automatically on first boot via the `docker-entrypoint-initdb.d` volume mount.

### 3. Run the backend

```bash
cargo run --release --bin soltrace
```

The API server starts on `http://localhost:3000`. The ingestion listener connects to Solana devnet and begins watching for transactions on registered wallets.

### 4. Run the dashboard

```bash
cd dashboard
npm install
npm run dev
```

The dashboard is available at `http://localhost:5173`.

### 5. Register a wallet

```bash
# Create an account
curl -X POST http://localhost:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "demo", "password": "demopass"}'

# Use the returned token to add a wallet
curl -X POST http://localhost:3000/wallets \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"pubkey": "<solana-wallet-address>", "label": "My Wallet"}'
```

---

## API Endpoints

### Public (no authentication required)

| Method | Path                     | Description                                    |
|--------|--------------------------|------------------------------------------------|
| GET    | `/health`                | Indexer status and last processed slot          |
| GET    | `/transfers`             | List transfers (query params: `wallet`, `direction`, `mint`, `limit`, `offset`) |
| GET    | `/balances?wallet=<pub>` | Token balances for a wallet                    |
| GET    | `/tx/{signature}`        | Look up a transaction by signature             |
| GET    | `/wallets`               | List watched wallets (scoped to user if authenticated) |
| GET    | `/backfill`              | List backfill jobs (optional `wallet` filter)  |
| GET    | `/backfill/{id}`         | Get a specific backfill job                    |
| GET    | `/webhooks`              | List all webhooks                              |
| GET    | `/webhooks/{id}`         | Get a specific webhook                         |
| GET    | `/idls`                  | List all registered program IDLs               |
| GET    | `/idls/{program_id}`     | Get IDL for a specific program                 |
| GET    | `/auth/google-client-id` | Get the configured Google OAuth client ID      |

### Protected (requires `Authorization: Bearer <token>`)

| Method | Path                     | Description                                    |
|--------|--------------------------|------------------------------------------------|
| POST   | `/wallets`               | Add a wallet to the watch list                 |
| DELETE | `/wallets/{pubkey}`      | Remove a wallet and its related data           |
| POST   | `/backfill`              | Start a historical backfill job for a wallet   |
| POST   | `/webhooks`              | Register a webhook with event filters          |
| DELETE | `/webhooks/{id}`         | Delete a webhook                               |
| POST   | `/idls`                  | Upload an Anchor IDL for dynamic decoding      |
| DELETE | `/idls/{program_id}`     | Remove a registered IDL                        |
| GET    | `/auth/me`               | Get the current authenticated user             |
| PUT    | `/auth/username`         | Update your username                           |

### Authentication

| Method | Path                     | Description                                    |
|--------|--------------------------|------------------------------------------------|
| POST   | `/auth/register`         | Create a new account (username + password)     |
| POST   | `/auth/login`            | Log in and receive a JWT                       |
| POST   | `/auth/google`           | Authenticate via Google ID token               |

---

## Configuration

Configuration is loaded from `config/default.toml` and can be overridden with environment variables using the `SOLTRACE__` prefix (double underscore separates nested keys).

### config/default.toml

```toml
[solana]
rpc_url = "https://api.devnet.solana.com"
ws_url = "wss://api.devnet.solana.com"

[database]
url = "postgres://soltrace:soltrace@localhost:5432/soltrace"
max_connections = 10

[redis]
url = "redis://127.0.0.1:6379"

[ingestion]
initial_backoff_ms = 500
max_backoff_ms = 30000
slot_cursor_key = "soltrace:last_slot"

[api]
host = "0.0.0.0"
port = 3000

[backfill]
rate_limit_ms = 200
batch_size = 100

[auth]
jwt_secret = "soltrace-dev-secret-change-in-production"
jwt_expiry_hours = 72
```

### Environment variable overrides

```bash
# Database
SOLTRACE__DATABASE__URL=postgres://user:pass@host:5432/soltrace

# Redis
SOLTRACE__REDIS__URL=redis://host:6379

# Solana RPC
SOLTRACE__SOLANA__RPC_URL=https://api.mainnet-beta.solana.com
SOLTRACE__SOLANA__WS_URL=wss://api.mainnet-beta.solana.com

# API
SOLTRACE__API__PORT=8080

# Auth
SOLTRACE__AUTH__JWT_SECRET=your-production-secret
SOLTRACE__AUTH__JWT_EXPIRY_HOURS=72
SOLTRACE__AUTH__GOOGLE_CLIENT_ID=your-google-client-id.apps.googleusercontent.com
```

---

## Deployment

### Backend (Railway)

1. Connect your GitHub repository to [Railway](https://railway.app).
2. Railway will detect the `Dockerfile` and build automatically.
3. Add a PostgreSQL and Redis service in your Railway project.
4. Set the following environment variables on the backend service:

| Variable                         | Description                          |
|----------------------------------|--------------------------------------|
| `SOLTRACE__DATABASE__URL`        | Railway-provided PostgreSQL URL      |
| `SOLTRACE__REDIS__URL`           | Railway-provided Redis URL           |
| `SOLTRACE__SOLANA__RPC_URL`      | Solana RPC endpoint                  |
| `SOLTRACE__SOLANA__WS_URL`       | Solana WebSocket endpoint            |
| `SOLTRACE__AUTH__JWT_SECRET`     | A strong random secret for JWT signing |
| `SOLTRACE__AUTH__GOOGLE_CLIENT_ID` | Google OAuth client ID (optional)  |
| `PORT`                           | Railway sets this automatically      |

5. The backend exposes port 3000 by default. Override with `SOLTRACE__API__PORT` if needed.

### Frontend (Vercel)

1. Import the `dashboard/` directory as a new project on [Vercel](https://vercel.com).
2. Set the root directory to `dashboard`.
3. Framework preset: **Next.js**.
4. Set the following environment variable:

| Variable                     | Description                                |
|------------------------------|--------------------------------------------|
| `NEXT_PUBLIC_API_URL`        | URL of the deployed backend (e.g., `https://soltrace-backend.up.railway.app`) |

5. If using Google OAuth, add your Vercel production domain to the authorized JavaScript origins in the Google Cloud Console.

---

## Screenshots

### Dashboard Overview
![Dashboard Overview](docs/screenshots/dashboard.PNG)

### Transfers
![Transfers](docs/screenshots/transfers.PNG)

### Wallets
![Wallets](docs/screenshots/wallets.PNG)

### Indexer Health
![Indexer Health](docs/screenshots/indexer-health.PNG)

### Sign In
![Sign In](docs/screenshots/sign-in.PNG)

---

## License

This project is licensed under the [MIT License](LICENSE).
