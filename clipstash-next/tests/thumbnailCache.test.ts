import { describe, expect, it, vi } from "vitest";
import { ThumbnailCache } from "../src/thumbnailCache";

const bytes = (length: number) => new Uint8Array(length).fill(1);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

describe("ThumbnailCache", () => {
  it("deduplicates concurrent acquires and retains one URL until cleared", async () => {
    const create = vi.fn(() => "blob:one");
    const revoke = vi.fn();
    const load = vi.fn().mockResolvedValue(bytes(3));
    const cache = new ThumbnailCache(100, 4, create, revoke);

    const first = cache.acquire("same", load);
    const second = cache.acquire("same", load);
    await expect(first.result).resolves.toBe("blob:one");
    await expect(second.result).resolves.toBe("blob:one");
    expect(load).toHaveBeenCalledTimes(1);
    expect(create).toHaveBeenCalledTimes(1);
    expect(cache.retainedBytes).toBe(3);

    first.release();
    second.release();
    cache.clear();
    expect(revoke).toHaveBeenCalledWith("blob:one");
    expect(cache.retainedBytes).toBe(0);
  });

  it("limits loaders to the configured concurrency and starts queued work in order", async () => {
    const pending = Array.from({ length: 5 }, () => deferred<Uint8Array>());
    const loads = pending.map((request) => vi.fn(() => request.promise));
    const cache = new ThumbnailCache(100, 4, (value) => `blob:${value[0]}`, vi.fn());
    const leases = loads.map((load, index) => cache.acquire(`key-${index}`, load));

    expect(loads.slice(0, 4).map((load) => load.mock.calls.length)).toEqual([1, 1, 1, 1]);
    expect(loads[4]).not.toHaveBeenCalled();
    pending.slice(0, 4).forEach((request, index) => request.resolve(new Uint8Array([index])));
    await Promise.all(leases.slice(0, 4).map((lease) => lease.result));
    expect(loads[4]).toHaveBeenCalledTimes(1);
    pending[4].resolve(new Uint8Array([4]));
    await expect(leases[4].result).resolves.toBe("blob:4");
  });

  it("cancels a queued lease before its loader starts", async () => {
    const firstPending = deferred<Uint8Array>();
    const firstLoad = vi.fn(() => firstPending.promise);
    const queuedLoad = vi.fn().mockResolvedValue(bytes(1));
    const cache = new ThumbnailCache(100, 1, vi.fn(() => "blob:one"), vi.fn());
    const first = cache.acquire("first", firstLoad);
    const queued = cache.acquire("queued", queuedLoad);
    queued.release();

    firstPending.resolve(bytes(1));
    await expect(first.result).resolves.toBe("blob:one");
    await expect(queued.result).rejects.toThrow("Thumbnail request released");
    expect(queuedLoad).not.toHaveBeenCalled();
  });

  it("evicts the least recently used released URL and reloads it on demand", async () => {
    let createCount = 0;
    const create = vi.fn((value: Uint8Array) => `blob:${value[0]}:${++createCount}`);
    const revoke = vi.fn();
    const cache = new ThumbnailCache(4, 4, create, revoke);
    const firstLoad = vi.fn().mockResolvedValue(new Uint8Array([1, 1, 1]));
    const secondLoad = vi.fn().mockResolvedValue(new Uint8Array([2, 2, 2]));

    const first = cache.acquire("first", firstLoad);
    await first.result;
    first.release();
    const second = cache.acquire("second", secondLoad);
    await second.result;
    second.release();

    expect(revoke).toHaveBeenCalledWith("blob:1:1");
    expect(cache.retainedBytes).toBe(3);
    const reloaded = cache.acquire("first", firstLoad);
    await expect(reloaded.result).resolves.toBe("blob:1:3");
    expect(firstLoad).toHaveBeenCalledTimes(2);
  });

  it("clears completed URLs and does not create or retain an in-flight released URL", async () => {
    const create = vi.fn(() => "blob:late");
    const revoke = vi.fn();
    const pending = deferred<Uint8Array>();
    const cache = new ThumbnailCache(100, 4, create, revoke);
    const lease = cache.acquire("late", () => pending.promise);

    lease.release();
    cache.clear();
    pending.resolve(bytes(2));
    await expect(lease.result).rejects.toThrow("Thumbnail request released");
    expect(create).not.toHaveBeenCalled();
    expect(revoke).not.toHaveBeenCalled();
    expect(cache.retainedBytes).toBe(0);
  });
});
