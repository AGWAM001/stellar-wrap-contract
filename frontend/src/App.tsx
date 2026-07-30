import { FormEvent, useCallback, useState } from "react";
import {
  errorMessage,
  formatTimestamp,
  parseHexBytes,
  shortAddress,
  validatePeriod,
} from "./lib/format";
import { connectWallet } from "./lib/freighter";
import { getWrap, loadDashboard, mintWrap, validateConfig } from "./lib/stellar";
import type {
  Dashboard,
  NetworkConfig,
  WalletSession,
  WrapRecord,
} from "./lib/types";

const TESTNET_PASSPHRASE = "Test SDF Network ; September 2015";
const DEFAULT_RPC_URL = "https://soroban-testnet.stellar.org";

type BusyAction = "connect" | "refresh" | "search" | "mint" | null;

const initialDraft: NetworkConfig = {
  contractId: import.meta.env.VITE_STELLAR_CONTRACT_ID ?? "",
  rpcUrl: import.meta.env.VITE_STELLAR_RPC_URL ?? DEFAULT_RPC_URL,
  networkPassphrase:
    import.meta.env.VITE_STELLAR_NETWORK_PASSPHRASE ?? TESTNET_PASSPHRASE,
};

function WrapCard({
  record,
  title,
}: {
  record: WrapRecord;
  title: string;
}) {
  return (
    <article className="wrap-card">
      <div className="wrap-card__heading">
        <div>
          <span className="eyebrow">{title}</span>
          <h3>{record.archetype}</h3>
        </div>
        <span className="period-pill">{record.period.toString()}</span>
      </div>
      <dl className="record-grid">
        <div>
          <dt>Minted</dt>
          <dd>{formatTimestamp(record.timestamp)}</dd>
        </div>
        <div>
          <dt>Data hash</dt>
          <dd className="hash-value" title={record.dataHash}>
            {record.dataHash}
          </dd>
        </div>
      </dl>
    </article>
  );
}

function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      {children}
      {hint ? <span className="field__hint">{hint}</span> : null}
    </div>
  );
}

export default function App() {
  const [draft, setDraft] = useState(initialDraft);
  const [config, setConfig] = useState<NetworkConfig | null>(null);
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [searchPeriod, setSearchPeriod] = useState("");
  const [searchResult, setSearchResult] = useState<
    WrapRecord | null | undefined
  >(undefined);
  const [mintPeriod, setMintPeriod] = useState("");
  const [archetype, setArchetype] = useState("");
  const [dataHash, setDataHash] = useState("");
  const [signature, setSignature] = useState("");
  const [transactionHash, setTransactionHash] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState<BusyAction>(null);

  const clearMessages = () => {
    setError("");
    setNotice("");
  };

  const refresh = useCallback(
    async (
      activeConfig: NetworkConfig | null = config,
      activeWallet: WalletSession | null = wallet,
    ) => {
      if (!activeConfig || !activeWallet) {
        return;
      }
      if (
        activeWallet.networkPassphrase !== activeConfig.networkPassphrase
      ) {
        throw new Error(
          `Freighter is on ${activeWallet.network}. Switch it to the configured network and reconnect.`,
        );
      }

      setDashboard(await loadDashboard(activeConfig, activeWallet.address));
    },
    [config, wallet],
  );

  const handleConfigure = (event: FormEvent) => {
    event.preventDefault();
    clearMessages();
    try {
      const nextConfig = validateConfig(draft);
      setConfig(nextConfig);
      setDashboard(null);
      setSearchResult(undefined);
      setTransactionHash("");
      setNotice("Contract configuration applied. Connect Freighter to continue.");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const handleConnect = async () => {
    if (!config) {
      return;
    }
    clearMessages();
    setBusy("connect");
    try {
      const nextWallet = await connectWallet();
      if (nextWallet.networkPassphrase !== config.networkPassphrase) {
        throw new Error(
          `Freighter is on ${nextWallet.network}. Switch it to the configured network and reconnect.`,
        );
      }
      setWallet(nextWallet);
      await refresh(config, nextWallet);
      setNotice("Freighter connected.");
    } catch (cause) {
      setWallet(null);
      setDashboard(null);
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const handleRefresh = async () => {
    clearMessages();
    setBusy("refresh");
    try {
      await refresh();
      setNotice("On-chain data refreshed.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const handleSearch = async (event: FormEvent) => {
    event.preventDefault();
    if (!config || !wallet) {
      return;
    }
    clearMessages();
    setSearchResult(undefined);
    setBusy("search");
    try {
      const period = validatePeriod(searchPeriod);
      setSearchResult(await getWrap(config, wallet.address, period));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const handleMint = async (event: FormEvent) => {
    event.preventDefault();
    if (!config || !wallet) {
      return;
    }
    clearMessages();
    setTransactionHash("");
    setBusy("mint");
    try {
      const period = validatePeriod(mintPeriod);
      if (!/^[A-Za-z0-9_]{1,32}$/.test(archetype)) {
        throw new Error(
          "Archetype must be 1–32 letters, numbers, or underscores.",
        );
      }
      const hash = await mintWrap(config, wallet.address, {
        period,
        archetype,
        dataHash: parseHexBytes(dataHash, 32, "Data hash"),
        signature: parseHexBytes(signature, 64, "Admin signature"),
      });
      setTransactionHash(hash);
      setNotice("Wrap minted and confirmed on-chain.");
      await refresh(config, wallet);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const isBusy = busy !== null;

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="#top" aria-label="Stellar Wrap home">
          <span className="brand__mark">W</span>
          <span>
            <strong>Stellar Wrap</strong>
            <small>On-chain registry</small>
          </span>
        </a>
        <div className="wallet-area">
          {wallet ? (
            <span className="wallet-chip" title={wallet.address}>
              <span className="status-dot" />
              {shortAddress(wallet.address)}
            </span>
          ) : (
            <button
              className="button button--primary"
              type="button"
              onClick={handleConnect}
              disabled={!config || isBusy}
            >
              {busy === "connect" ? "Connecting…" : "Connect Freighter"}
            </button>
          )}
        </div>
      </header>

      <main id="top">
        <section className="hero">
          <div className="hero__copy">
            <span className="eyebrow">Proof that stays with you</span>
            <h1>Your Stellar story, wrapped on-chain.</h1>
            <p>
              Connect Freighter to inspect non-transferable wrap records and mint
              a signed new entry to the Stellar Wrap registry.
            </p>
          </div>
          <div className="hero__orb" aria-hidden="true">
            <span>WRAP</span>
          </div>
        </section>

        <div className="message-stack" aria-live="polite">
          {error ? <div className="message message--error">{error}</div> : null}
          {notice ? (
            <div className="message message--success">{notice}</div>
          ) : null}
        </div>

        <section className="panel setup-panel" aria-labelledby="setup-title">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Step 01</span>
              <h2 id="setup-title">Choose your contract</h2>
            </div>
            {config ? <span className="status-badge">Configured</span> : null}
          </div>
          <form className="config-form" onSubmit={handleConfigure}>
            <Field id="contract-id" label="Contract ID">
              <input
                id="contract-id"
                value={draft.contractId}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    contractId: event.target.value,
                  }))
                }
                placeholder="C…"
                autoComplete="off"
                spellCheck={false}
              />
            </Field>
            <details className="advanced-config">
              <summary>Network settings</summary>
              <div className="advanced-config__grid">
                <Field id="rpc-url" label="Soroban RPC URL">
                  <input
                    id="rpc-url"
                    type="url"
                    value={draft.rpcUrl}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        rpcUrl: event.target.value,
                      }))
                    }
                  />
                </Field>
                <Field id="network-passphrase" label="Network passphrase">
                  <input
                    id="network-passphrase"
                    value={draft.networkPassphrase}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        networkPassphrase: event.target.value,
                      }))
                    }
                  />
                </Field>
              </div>
            </details>
            <button className="button button--secondary" type="submit">
              Use contract
            </button>
          </form>
        </section>

        {wallet && dashboard ? (
          <>
            <section className="dashboard" aria-labelledby="overview-title">
              <div className="section-heading">
                <div>
                  <span className="eyebrow">Step 02</span>
                  <h2 id="overview-title">Registry overview</h2>
                </div>
                <button
                  className="button button--ghost"
                  type="button"
                  onClick={handleRefresh}
                  disabled={isBusy}
                >
                  {busy === "refresh" ? "Refreshing…" : "Refresh"}
                </button>
              </div>
              <div className="stat-grid">
                <article className="stat-card">
                  <span>Your wraps</span>
                  <strong>{dashboard.balance.toString().padStart(2, "0")}</strong>
                  <small>Non-transferable records</small>
                </article>
                <article className="stat-card">
                  <span>Contract</span>
                  <strong className="status-word">
                    {dashboard.health.initialized ? "Ready" : "Offline"}
                  </strong>
                  <small>
                    Admin {dashboard.health.hasAdmin ? "set" : "missing"} · Key{" "}
                    {dashboard.health.hasSigningKey ? "set" : "missing"}
                  </small>
                </article>
                <article className="stat-card">
                  <span>Network</span>
                  <strong className="status-word">{wallet.network}</strong>
                  <small>{shortAddress(wallet.address)}</small>
                </article>
              </div>
              {dashboard.latestWrap ? (
                <WrapCard record={dashboard.latestWrap} title="Latest wrap" />
              ) : (
                <div className="empty-state">
                  <span className="empty-state__mark">✦</span>
                  <div>
                    <h3>No wraps yet</h3>
                    <p>Your first confirmed wrap will appear here.</p>
                  </div>
                </div>
              )}
            </section>

            <section className="action-grid">
              <article className="panel" aria-labelledby="find-title">
                <span className="eyebrow">Explore</span>
                <h2 id="find-title">Find a wrap</h2>
                <p className="panel__intro">
                  Query your wallet for a specific reporting period.
                </p>
                <form className="stacked-form" onSubmit={handleSearch}>
                  <Field id="search-period" label="Period" hint="Format: YYYYMM">
                    <input
                      id="search-period"
                      inputMode="numeric"
                      value={searchPeriod}
                      onChange={(event) => setSearchPeriod(event.target.value)}
                      placeholder="202607"
                      maxLength={6}
                    />
                  </Field>
                  <button
                    className="button button--secondary"
                    disabled={isBusy}
                    type="submit"
                  >
                    {busy === "search" ? "Searching…" : "Find record"}
                  </button>
                </form>
                {searchResult === null ? (
                  <p className="inline-empty">No wrap exists for that period.</p>
                ) : searchResult ? (
                  <WrapCard record={searchResult} title="Search result" />
                ) : null}
              </article>

              <article className="panel mint-panel" aria-labelledby="mint-title">
                <span className="eyebrow">Create</span>
                <h2 id="mint-title">Mint a new wrap</h2>
                <p className="panel__intro">
                  Enter the commitment and signature issued by the trusted wrap
                  service. Freighter will preview and approve the transaction.
                </p>
                <form className="stacked-form" onSubmit={handleMint}>
                  <div className="form-row">
                    <Field id="mint-period" label="Period" hint="YYYYMM">
                      <input
                        id="mint-period"
                        inputMode="numeric"
                        value={mintPeriod}
                        onChange={(event) => setMintPeriod(event.target.value)}
                        placeholder="202607"
                        maxLength={6}
                      />
                    </Field>
                    <Field id="archetype" label="Archetype">
                      <input
                        id="archetype"
                        value={archetype}
                        onChange={(event) => setArchetype(event.target.value)}
                        placeholder="builder"
                        maxLength={32}
                      />
                    </Field>
                  </div>
                  <Field
                    id="data-hash"
                    label="Data hash"
                    hint="32-byte SHA-256 value in hexadecimal"
                  >
                    <textarea
                      id="data-hash"
                      value={dataHash}
                      onChange={(event) => setDataHash(event.target.value)}
                      placeholder="64 hexadecimal characters"
                      rows={2}
                      spellCheck={false}
                    />
                  </Field>
                  <Field
                    id="signature"
                    label="Admin signature"
                    hint="64-byte Ed25519 signature in hexadecimal"
                  >
                    <textarea
                      id="signature"
                      value={signature}
                      onChange={(event) => setSignature(event.target.value)}
                      placeholder="128 hexadecimal characters"
                      rows={3}
                      spellCheck={false}
                    />
                  </Field>
                  <div className="security-note">
                    <span aria-hidden="true">◎</span>
                    <p>
                      This app never asks for secret keys. Freighter signs only
                      after contract simulation succeeds.
                    </p>
                  </div>
                  <button
                    className="button button--primary button--wide"
                    disabled={isBusy || !dashboard.health.initialized}
                    type="submit"
                  >
                    {busy === "mint" ? "Confirming on-chain…" : "Review & mint"}
                  </button>
                </form>
                {transactionHash ? (
                  <div className="transaction-result">
                    <span>Confirmed transaction</span>
                    <code>{transactionHash}</code>
                  </div>
                ) : null}
              </article>
            </section>
          </>
        ) : (
          <section className="connect-gate">
            <span className="connect-gate__icon" aria-hidden="true">
              ↗
            </span>
            <div>
              <h2>Connect to see your wraps</h2>
              <p>
                Configure the deployed contract, then approve this dApp in
                Freighter. Your keys always stay inside the wallet.
              </p>
            </div>
            <button
              className="button button--primary"
              type="button"
              onClick={handleConnect}
              disabled={!config || isBusy}
            >
              {busy === "connect" ? "Connecting…" : "Connect Freighter"}
            </button>
          </section>
        )}
      </main>

      <footer>
        <span>Stellar Wrap Registry</span>
        <span>Built for Soroban · Secured by Freighter</span>
      </footer>
    </div>
  );
}
