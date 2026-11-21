import { useCallback, useEffect, useRef, useState } from "react";
import { call } from "./api";
import type { CasualJob } from "./types";

/**
 * Acompanha uma tarefa do Casual (configurar, adicionar, aplicar, reverter).
 * `track(job)` começa a acompanhar; a tarefa continua rodando no núcleo mesmo se a tela for trocada.
 */
export function useJob(onFinish?: (job: CasualJob) => void) {
  const [job, setJob] = useState<CasualJob | null>(null);
  const finish = useRef(onFinish);
  finish.current = onFinish;
  const running = job?.state === "running";
  const id = job?.id;

  useEffect(() => {
    if (!running) return;
    let alive = true;
    const timer = setInterval(() => {
      call<CasualJob | null>("casual_job")
        .then((j) => {
          if (!alive || !j || j.id !== id) return;
          setJob(j);
          if (j.state !== "running") finish.current?.(j);
        })
        .catch(() => undefined);
    }, 700);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [running, id]);

  const track = useCallback((j: CasualJob) => setJob(j), []);
  const dismiss = useCallback(() => setJob(null), []);
  return { job, track, dismiss };
}
