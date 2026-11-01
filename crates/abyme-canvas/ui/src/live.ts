import { useEffect, useState } from "react";

import { coalesced } from "./boards";
import type { AbymeClient } from "./client";

/**
 * The result of `load`, starting from `initial` and asked for again whenever
 * the project's files change, and whenever `alsoAskWhen` calls what it is given,
 * until the function it returns stops it. At most one run is in flight, however
 * often it is asked. `load` and `alsoAskWhen` must keep their identity between
 * renders, since a new one would start a run beside the one in flight; a result
 * that arrives after the component is gone is dropped.
 */
export function useLive<T>(
  client: AbymeClient,
  load: () => Promise<T>,
  initial: T,
  alsoAskWhen?: (ask: () => void) => () => void,
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
    const stopAsking = alsoAskWhen?.(() => void refresh());
    return () => {
      current = false;
      void listening.then((stop) => stop());
      stopAsking?.();
    };
  }, [client, load, alsoAskWhen]);

  return state;
}
