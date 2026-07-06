export type BuyerThreadStatus =
  | "purchase_seen"
  | "waiting_fulfill"
  | "ready_to_download"
  | "settled"
  | "refunded"
  | "blocked";

export type BuyerThread = {
  id: string;
  saleId: `0x${string}`;
  channel: `0x${string}`;
  buyer: `0x${string}`;
  title: string;
  txHash: `0x${string}`;
  status: BuyerThreadStatus;
  updatedAt: number;
};

const DB_NAME = "fair-file-marketplace";
const STORE = "buyer_threads";

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(STORE, { keyPath: "id" });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

export async function listLocalThreads(): Promise<BuyerThread[]> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, "readonly");
    const request = tx.objectStore(STORE).getAll();
    request.onsuccess = () => resolve(request.result as BuyerThread[]);
    request.onerror = () => reject(request.error);
  });
}

export async function upsertLocalThread(thread: BuyerThread): Promise<void> {
  const db = await openDb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(thread);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}
