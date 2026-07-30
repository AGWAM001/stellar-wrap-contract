import type { ContractHealth, WrapRecord } from "./types";

const HEX_PATTERN = /^(?:0x)?[0-9a-fA-F]+$/;

export function parseHexBytes(
  value: string,
  expectedBytes: number,
  fieldName: string,
): Uint8Array {
  const trimmed = value.trim();
  if (!HEX_PATTERN.test(trimmed)) {
    throw new Error(`${fieldName} must contain only hexadecimal characters.`);
  }

  const hex = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (hex.length !== expectedBytes * 2) {
    throw new Error(`${fieldName} must be exactly ${expectedBytes} bytes.`);
  }

  return Uint8Array.from(
    Array.from({ length: expectedBytes }, (_, index) =>
      Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16),
    ),
  );
}

export function validatePeriod(value: string): bigint {
  if (!/^\d{6}$/.test(value)) {
    throw new Error("Period must use the YYYYMM format.");
  }

  const year = Number(value.slice(0, 4));
  const month = Number(value.slice(4));
  if (year < 2024 || year > 2100 || month < 1 || month > 12) {
    throw new Error("Period must be a valid month from 2024 through 2100.");
  }

  return BigInt(value);
}

export function normalizeHealth(value: unknown): ContractHealth {
  const health = value as Record<string, unknown>;
  return {
    initialized: Boolean(health.initialized),
    hasAdmin: Boolean(health.has_admin),
    hasSigningKey: Boolean(health.has_signing_key),
  };
}

export function normalizeWrap(value: unknown): WrapRecord | null {
  if (value === null || value === undefined) {
    return null;
  }

  const record = value as Record<string, unknown>;
  const dataHash = record.data_hash;
  if (
    !ArrayBuffer.isView(dataHash) ||
    !("length" in dataHash) ||
    dataHash.byteLength !== dataHash.length ||
    dataHash.byteLength !== 32
  ) {
    throw new Error("The contract returned an invalid data hash.");
  }
  const hashBytes = new Uint8Array(
    dataHash.buffer,
    dataHash.byteOffset,
    dataHash.byteLength,
  );

  return {
    timestamp: BigInt(record.timestamp as bigint | number | string),
    dataHash: toHex(hashBytes),
    archetype: String(record.archetype),
    period: BigInt(record.period as bigint | number | string),
  };
}

export function toHex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function shortAddress(address: string): string {
  return `${address.slice(0, 6)}…${address.slice(-6)}`;
}

export function formatTimestamp(timestamp: bigint): string {
  if (timestamp === 0n) {
    return "Unknown";
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(Number(timestamp) * 1000));
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return "Something unexpected happened. Please try again.";
}
