import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { readImageThumbnailBytes } from "./api/legacy";
import type { LegacyMessageImage } from "./api/types";
import { ThumbnailCache } from "./thumbnailCache";

const CacheContext = createContext<ThumbnailCache | null>(null);

export function ThumbnailProvider({ children }: { children: ReactNode }) {
  const [cache] = useState(() => new ThumbnailCache());
  useEffect(() => () => cache.clear(), [cache]);
  return <CacheContext.Provider value={cache}>{children}</CacheContext.Provider>;
}

export function useThumbnail(image: LegacyMessageImage | null) {
  const cache = useContext(CacheContext);
  const element = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(typeof IntersectionObserver === "undefined");
  const [source, setSource] = useState<{ key: string; url?: string; failed?: boolean } | null>(null);
  const key = image?.path ?? "";

  useEffect(() => {
    const node = element.current;
    if (!node || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) setSource(null);
      setNear(entry.isIntersecting);
    }, {
      root: node.closest(".message-list"), rootMargin: "160px",
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!cache || !image?.exists || !near) return;
    let alive = true;
    const lease = cache.acquire(key, () => readImageThumbnailBytes(image.filename, image.path));
    lease.result.then(
      (url) => { if (alive) setSource({ key, url }); },
      () => { if (alive) setSource({ key, failed: true }); },
    );
    return () => { alive = false; lease.release(); };
  }, [cache, image?.filename, image?.exists, key, near]);

  // Detach offscreen DOM from the URL before its lease can be evicted.
  return { element, src: near && source?.key === key ? source.url ?? "" : "",
    failed: near && source?.key === key && Boolean(source.failed) };
}
