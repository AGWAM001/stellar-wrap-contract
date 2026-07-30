# Stellar Wrap dApp

React and Freighter client for the deployed Stellar Wrap Soroban contract.

## What it supports

- Freighter connection and active-network verification
- configurable contract ID, RPC endpoint, and network passphrase
- contract health, wallet wrap count, and latest-wrap queries
- lookup by `YYYYMM` period
- `mint_wrap` simulation, Freighter approval, submission, and confirmation
- responsive and keyboard-accessible status, error, and empty states

The mint form accepts only public commitment material: a SHA-256 data hash and
the Ed25519 signature produced by the trusted wrap service. It never asks for an
admin signing key or a wallet secret.

## Local setup

Node.js 20.19 or later is required.

```bash
cd frontend
cp .env.example .env
npm ci
npm run dev
```

Set `VITE_STELLAR_CONTRACT_ID` in `.env`, or paste a deployed contract ID in the
app. The defaults point to Stellar testnet. If you change the RPC endpoint or
passphrase, Freighter must be switched to the same network before the app will
read or submit anything.

Configuration values are public frontend settings. Do not put secret keys,
admin signing keys, or credentials in `.env`; all `VITE_` values are bundled
into the browser build. Remote RPC endpoints must use HTTPS, and RPC URLs with
embedded credentials are rejected. Plain HTTP is accepted only for loopback
addresses during local development.

## Architecture

```text
React UI
  ├── Freighter adapter ── wallet permission, network, transaction signature
  └── Stellar adapter
        ├── simulate read-only contract calls
        └── simulate → assemble → sign → submit → confirm mint_wrap
```

- `src/App.tsx` owns the explicit configuration, wallet, query, and transaction
  states.
- `src/lib/freighter.ts` is the only module that calls the Freighter API.
- `src/lib/stellar.ts` validates network configuration and owns Soroban RPC/XDR
  conversion.
- `src/lib/format.ts` validates user input and converts contract-native values
  into display models.

Read calls are simulated through RPC and decoded from `ScVal`. Minting first
simulates the complete invocation, then assembles Soroban resource data, asks
Freighter to sign the prepared XDR, submits it, and waits for a success result.
The UI refreshes contract state only after confirmation.

## Verification

```bash
npm run typecheck
npm run lint
npm test
npm run build
npm run audit
```

Tests mock wallet and network boundaries; they do not require Freighter,
credentials, a funded account, or a live contract.
