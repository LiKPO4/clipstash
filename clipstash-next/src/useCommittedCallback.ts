import { useCallback, useLayoutEffect, useRef } from "react";

/** Stable event handler that calls the latest committed render, never a stale draft. */
export function useCommittedCallback<Args extends unknown[], Result>(callback: (...args: Args) => Result) {
  const current = useRef(callback);
  useLayoutEffect(() => { current.current = callback; });
  return useCallback((...args: Args) => current.current(...args), []);
}
