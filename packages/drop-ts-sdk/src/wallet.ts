import { createWalletClient, custom, type WalletClient } from "viem";
import { arbitrumSepolia } from "viem/chains";
import { TRUSTDROP_CHAIN_ID_HEX } from "./config";

export type BrowserWallet = {
  account: `0x${string}`;
  client: WalletClient;
};

type EthereumProvider = {
  request(args: { method: string; params?: unknown[] }): Promise<unknown>;
};

declare global {
  interface Window {
    ethereum?: EthereumProvider;
  }
}

export function hasInjectedWallet(): boolean {
  return typeof window !== "undefined" && !!window.ethereum;
}

export async function connectWallet(): Promise<BrowserWallet> {
  if (!window.ethereum) throw new Error("No injected wallet found");
  const accounts = (await window.ethereum.request({ method: "eth_requestAccounts" })) as `0x${string}`[];
  const account = accounts[0];
  if (!account) throw new Error("No wallet account selected");

  const chainId = (await window.ethereum.request({ method: "eth_chainId" })) as string;
  if (chainId.toLowerCase() !== TRUSTDROP_CHAIN_ID_HEX) {
    await window.ethereum.request({
      method: "wallet_switchEthereumChain",
      params: [{ chainId: TRUSTDROP_CHAIN_ID_HEX }],
    });
  }

  return {
    account,
    client: createWalletClient({
      account,
      chain: arbitrumSepolia,
      transport: custom(window.ethereum),
    }),
  };
}

