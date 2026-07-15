import { createWalletClient, custom, type WalletClient } from "viem";
import { arbitrumSepolia } from "viem/chains";
import { TRUSTDROP_CHAIN_ID_HEX } from "./config";

export type BrowserWallet = {
  account: `0x${string}`;
  client: WalletClient;
};

type EthereumProvider = {
  request(args: { method: string; params?: unknown[] }): Promise<unknown>;
  on?(event: "accountsChanged", listener: (accounts: string[]) => void): void;
  on?(event: "chainChanged", listener: (chainId: string) => void): void;
  removeListener?(event: "accountsChanged", listener: (accounts: string[]) => void): void;
  removeListener?(event: "chainChanged", listener: (chainId: string) => void): void;
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
  await window.ethereum.request({
    method: "wallet_requestPermissions",
    params: [{ eth_accounts: {} }],
  });
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

export async function walletFromAccount(account: `0x${string}`): Promise<BrowserWallet> {
  if (!window.ethereum) throw new Error("No injected wallet found");
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

export function onWalletAccountsChanged(listener: (accounts: `0x${string}`[]) => void): () => void {
  const provider = window.ethereum;
  if (!provider?.on) return () => {};
  const wrapped = (accounts: string[]) => listener(accounts as `0x${string}`[]);
  provider.on("accountsChanged", wrapped);
  return () => provider.removeListener?.("accountsChanged", wrapped);
}

export function onWalletChainChanged(listener: (chainId: string) => void): () => void {
  const provider = window.ethereum;
  if (!provider?.on) return () => {};
  provider.on("chainChanged", listener);
  return () => provider.removeListener?.("chainChanged", listener);
}
