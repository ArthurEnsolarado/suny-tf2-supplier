// Em release, não abre a janela preta do console no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use suny_core::{commands, gamebanana::Client};

struct AppState {
    client: Client,
}

/// Único ponto de entrada da interface. A lista de comandos permitidos fica em
/// `suny_core::commands::dispatch`; qualquer nome fora dela volta como erro.
#[tauri::command]
async fn call(
    state: tauri::State<'_, AppState>,
    name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    commands::dispatch(&state.client, &name, args).await
}

/// Fecha o app. A interface chama isto depois de abrir o instalador de uma atualização.
#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    let client = Client::new(env!("CARGO_PKG_VERSION")).expect("não consegui criar o cliente HTTP");
    tauri::Builder::default()
        .manage(AppState { client })
        .invoke_handler(tauri::generate_handler![call, exit_app])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
