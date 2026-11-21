// Ponte com o núcleo Rust.
// No app (Tauri) usa `invoke`. No navegador de desenvolvimento fala com o devserver local
// (`cargo run -p suny-core --example devserver`), que roda o MESMO código do núcleo.

const DEV_SERVER = "http://127.0.0.1:1421/api";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export class ApiError extends Error {}

/** Fecha o app (usado depois de abrir o instalador de uma atualização). No navegador de desenvolvimento não faz nada. */
export async function quitApp(): Promise<void> {
  if (!inTauri) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("exit_app");
}

export async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<T>("call", { name, args });
    } catch (e) {
      throw new ApiError(typeof e === "string" ? e : String(e));
    }
  }

  let res: Response;
  try {
    res = await fetch(`${DEV_SERVER}/${name}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(args),
    });
  } catch {
    throw new ApiError(
      "Não consegui falar com o núcleo. Em desenvolvimento, rode: cargo run -p suny-core --example devserver",
    );
  }
  const data: unknown = await res.json().catch(() => null);
  if (!res.ok) {
    const msg = (data as { error?: string } | null)?.error;
    throw new ApiError(msg ?? `Erro ${res.status}`);
  }
  return data as T;
}
