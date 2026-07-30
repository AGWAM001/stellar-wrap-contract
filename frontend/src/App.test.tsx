import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { connectWallet } from "./lib/freighter";
import { getWrap, loadDashboard, mintWrap, validateConfig } from "./lib/stellar";

vi.mock("./lib/freighter", () => ({
  connectWallet: vi.fn(),
}));

vi.mock("./lib/stellar", () => ({
  getWrap: vi.fn(),
  loadDashboard: vi.fn(),
  mintWrap: vi.fn(),
  validateConfig: vi.fn((config) => config),
}));

const wallet = {
  address: "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
  network: "TESTNET",
  networkPassphrase: "Test SDF Network ; September 2015",
};

const dashboard = {
  balance: 2n,
  health: {
    initialized: true,
    hasAdmin: true,
    hasSigningKey: true,
  },
  latestWrap: null,
};

async function configureAndConnect() {
  const user = userEvent.setup();
  vi.mocked(connectWallet).mockResolvedValue(wallet);
  vi.mocked(loadDashboard).mockResolvedValue(dashboard);

  await user.type(screen.getByLabelText("Contract ID"), "CVALID");
  await user.click(screen.getByRole("button", { name: "Use contract" }));
  await user.click(
    screen.getAllByRole("button", { name: "Connect Freighter" })[0],
  );

  await screen.findByRole("heading", { name: "Registry overview" });
  return user;
}

describe("App", () => {
  it("configures the contract, connects Freighter, and loads wallet data", async () => {
    render(<App />);

    await configureAndConnect();

    expect(validateConfig).toHaveBeenCalledWith(
      expect.objectContaining({ contractId: "CVALID" }),
    );
    expect(connectWallet).toHaveBeenCalledOnce();
    expect(loadDashboard).toHaveBeenCalledWith(
      expect.objectContaining({ contractId: "CVALID" }),
      wallet.address,
    );
    expect(screen.getByText("02")).toBeInTheDocument();
    expect(screen.getByText("Freighter connected.")).toBeInTheDocument();
  });

  it("blocks contract calls when a reporting period is invalid", async () => {
    render(<App />);
    const user = await configureAndConnect();

    await user.type(screen.getByLabelText("Period", { selector: "#search-period" }), "202613");
    await user.click(screen.getByRole("button", { name: "Find record" }));

    expect(
      await screen.findByText(
        "Period must be a valid month from 2024 through 2100.",
      ),
    ).toBeInTheDocument();
    expect(getWrap).not.toHaveBeenCalled();
  });

  it("shows an explicit empty state when a queried wrap is absent", async () => {
    vi.mocked(getWrap).mockResolvedValue(null);
    render(<App />);
    const user = await configureAndConnect();

    await user.type(screen.getByLabelText("Period", { selector: "#search-period" }), "202607");
    await user.click(screen.getByRole("button", { name: "Find record" }));

    expect(
      await screen.findByText("No wrap exists for that period."),
    ).toBeInTheDocument();
  });

  it("validates, submits, confirms, and refreshes a mint", async () => {
    vi.mocked(mintWrap).mockResolvedValue("abc123");
    render(<App />);
    const user = await configureAndConnect();

    await user.type(screen.getByLabelText("Period", { selector: "#mint-period" }), "202607");
    await user.type(screen.getByLabelText("Archetype"), "builder");
    await user.type(screen.getByLabelText("Data hash"), "11".repeat(32));
    await user.type(screen.getByLabelText("Admin signature"), "22".repeat(64));
    await user.click(screen.getByRole("button", { name: "Review & mint" }));

    await waitFor(() => expect(mintWrap).toHaveBeenCalledOnce());
    expect(mintWrap).toHaveBeenCalledWith(
      expect.objectContaining({ contractId: "CVALID" }),
      wallet.address,
      {
        period: 202607n,
        archetype: "builder",
        dataHash: new Uint8Array(32).fill(0x11),
        signature: new Uint8Array(64).fill(0x22),
      },
    );
    expect(await screen.findByText("abc123")).toBeInTheDocument();
    expect(
      screen.getByText("Wrap minted and confirmed on-chain."),
    ).toBeInTheDocument();
    expect(loadDashboard).toHaveBeenCalledTimes(2);
  });

  it("rejects a Freighter network mismatch before reading the contract", async () => {
    vi.mocked(connectWallet).mockResolvedValue({
      ...wallet,
      network: "PUBLIC",
      networkPassphrase: "Public Global Stellar Network ; September 2015",
    });
    render(<App />);
    const user = userEvent.setup();

    await user.type(screen.getByLabelText("Contract ID"), "CVALID");
    await user.click(screen.getByRole("button", { name: "Use contract" }));
    await user.click(
      screen.getAllByRole("button", { name: "Connect Freighter" })[0],
    );

    expect(
      await screen.findByText(
        "Freighter is on PUBLIC. Switch it to the configured network and reconnect.",
      ),
    ).toBeInTheDocument();
    expect(loadDashboard).not.toHaveBeenCalled();
  });
});
