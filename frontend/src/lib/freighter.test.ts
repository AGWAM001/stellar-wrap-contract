import {
  getNetwork,
  isConnected,
  requestAccess,
  signTransaction,
} from "@stellar/freighter-api";
import { connectWallet, signWithFreighter } from "./freighter";

vi.mock("@stellar/freighter-api", () => ({
  getNetwork: vi.fn(),
  isConnected: vi.fn(),
  requestAccess: vi.fn(),
  signTransaction: vi.fn(),
}));

const ADDRESS = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const PASSPHRASE = "Test SDF Network ; September 2015";

describe("Freighter adapter", () => {
  beforeEach(() => {
    vi.mocked(isConnected).mockResolvedValue({ isConnected: true });
    vi.mocked(requestAccess).mockResolvedValue({ address: ADDRESS });
    vi.mocked(getNetwork).mockResolvedValue({
      network: "TESTNET",
      networkPassphrase: PASSPHRASE,
    });
    vi.mocked(signTransaction).mockResolvedValue({
      signedTxXdr: "signed-xdr",
      signerAddress: ADDRESS,
    });
  });

  it("requests wallet access and returns the active account and network", async () => {
    await expect(connectWallet()).resolves.toEqual({
      address: ADDRESS,
      network: "TESTNET",
      networkPassphrase: PASSPHRASE,
    });
    expect(isConnected).toHaveBeenCalledOnce();
    expect(requestAccess).toHaveBeenCalledOnce();
    expect(getNetwork).toHaveBeenCalledOnce();
  });

  it("stops before requesting access when Freighter is unavailable", async () => {
    vi.mocked(isConnected).mockResolvedValue({ isConnected: false });

    await expect(connectWallet()).rejects.toThrow("Freighter was not detected");
    expect(requestAccess).not.toHaveBeenCalled();
  });

  it("reports rejected access without querying the network", async () => {
    vi.mocked(requestAccess).mockResolvedValue({ address: "" });

    await expect(connectWallet()).rejects.toThrow(
      "Wallet access was not approved",
    );
    expect(getNetwork).not.toHaveBeenCalled();
  });

  it("rejects an empty network response", async () => {
    vi.mocked(getNetwork).mockResolvedValue({
      network: "",
      networkPassphrase: "",
    });

    await expect(connectWallet()).rejects.toThrow(
      "Could not read the Freighter network",
    );
  });

  it("passes the account and passphrase when requesting a signature", async () => {
    await expect(
      signWithFreighter("unsigned-xdr", ADDRESS, PASSPHRASE),
    ).resolves.toBe("signed-xdr");
    expect(signTransaction).toHaveBeenCalledWith("unsigned-xdr", {
      address: ADDRESS,
      networkPassphrase: PASSPHRASE,
    });
  });

  it("rejects an empty signed transaction response", async () => {
    vi.mocked(signTransaction).mockResolvedValue({
      signedTxXdr: "",
      signerAddress: "",
    });

    await expect(
      signWithFreighter("unsigned-xdr", ADDRESS, PASSPHRASE),
    ).rejects.toThrow("Freighter did not sign the transaction");
  });
});
