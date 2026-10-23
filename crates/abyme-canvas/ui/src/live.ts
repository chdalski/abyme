import { useEffect, useState } from "react";

import { coalesced } from "./boards";
import type { AbymeClient } from "./client";

/**
 * The result of `load`, starting from `initial` and asked for again whenever
 * the project's files change. `load` must keep its identity between renders;
 * a result that arrives after the component is gone is dropped.
 */
export function useLive<T>(
  client: AbymeClient,
  load: () => Promise<T>,
  initial: T,
): T {
  const [state, setState] = useState(initial);

  useEffect(() => {
    let current = true;
    const refresh = coalesced(async () => {
      const next = await load();
      if (current) {
        setState(next);
      }
    });
    void refresh();
    const listening = client.onProjectChanged(() => void refresh());
    return () => {
      current = false;
      void listening.then((stop) => stop());
    };
  }, [client, load]);

  return state;
}
