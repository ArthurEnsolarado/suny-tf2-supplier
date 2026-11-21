import { useCallback, useEffect, useState } from "react";
import { call } from "./api";
import type { UpdateInfo } from "./types";

/** Consulta as releases estáveis do projeto. Com `auto`, confere uma vez ao abrir o app. */
export function useUpdate(auto: boolean) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | undefined>();

  const check = useCallback(() => {
    setChecking(true);
    setError(undefined);
    call<UpdateInfo>("update_check").then(
      (i) => {
        setInfo(i);
        setChecking(false);
      },
      (e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
        setChecking(false);
      },
    );
  }, []);

  useEffect(() => {
    if (auto) check();
  }, [auto, check]);

  return { info, checking, error, check };
}
