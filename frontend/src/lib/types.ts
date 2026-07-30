export type NetworkConfig = {
  contractId: string;
  rpcUrl: string;
  networkPassphrase: string;
};

export type WalletSession = {
  address: string;
  network: string;
  networkPassphrase: string;
};

export type ContractHealth = {
  initialized: boolean;
  hasAdmin: boolean;
  hasSigningKey: boolean;
};

export type WrapRecord = {
  timestamp: bigint;
  dataHash: string;
  archetype: string;
  period: bigint;
};

export type Dashboard = {
  balance: bigint;
  health: ContractHealth;
  latestWrap: WrapRecord | null;
};

export type MintInput = {
  period: bigint;
  archetype: string;
  dataHash: Uint8Array;
  signature: Uint8Array;
};
