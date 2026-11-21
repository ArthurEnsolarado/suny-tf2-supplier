//! Servidor SÓ PARA DESENVOLVIMENTO da interface.
//!
//! `cargo run -p suny-core --example devserver`
//!
//! Deixa abrir a interface num navegador comum (`npm run dev`, porta 1420) falando com o mesmo
//! núcleo do app, com dados reais da sua máquina e do GameBanana. Só aceita pedidos vindos da
//! própria interface de desenvolvimento e só escuta em 127.0.0.1. O app final não usa isto.

use std::io::Read;

use serde_json::{json, Value};
use suny_core::{commands, gamebanana::Client};

const PORT: u16 = 1421;
const ALLOWED_ORIGINS: &[&str] = &["http://localhost:1420", "http://127.0.0.1:1420"];

fn header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = Client::new("devserver").unwrap();
    let server = tiny_http::Server::http(("127.0.0.1", PORT)).expect("porta 1421 ocupada?");
    println!("devserver em http://127.0.0.1:{PORT}  (aceita só {ALLOWED_ORIGINS:?})");

    for mut req in server.incoming_requests() {
        let origin = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Origin"))
            .map(|h| h.value.as_str().to_string())
            .unwrap_or_default();
        let allowed = ALLOWED_ORIGINS.contains(&origin.as_str());

        let mut resp_headers = vec![header("Content-Type", "application/json; charset=utf-8")];
        if allowed {
            resp_headers.push(header("Access-Control-Allow-Origin", &origin));
            resp_headers.push(header("Access-Control-Allow-Headers", "content-type"));
            resp_headers.push(header("Vary", "Origin"));
        }

        if req.method() == &tiny_http::Method::Options {
            // Pedido de verificação (preflight) do navegador: responde com os mesmos cabeçalhos de permissão.
            let mut r = tiny_http::Response::empty(if allowed { 204 } else { 403 });
            if allowed {
                resp_headers.push(header("Access-Control-Allow-Methods", "POST, OPTIONS"));
                for h in resp_headers {
                    r = r.with_header(h);
                }
            }
            let _ = req.respond(r);
            continue;
        }
        if !allowed {
            let _ = req.respond(tiny_http::Response::from_string(r#"{"error":"origem não permitida"}"#).with_status_code(403));
            continue;
        }

        let name = req.url().trim_start_matches("/api/").split('?').next().unwrap_or("").to_string();
        let mut body = String::new();
        let _ = req.as_reader().take(1 << 20).read_to_string(&mut body);
        let args: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));

        let (status, payload) = match rt.block_on(commands::dispatch(&client, &name, args)) {
            Ok(v) => (200, v),
            Err(e) => (400, json!({ "error": e })),
        };
        println!("{status} {name}");

        let mut r = tiny_http::Response::from_string(payload.to_string()).with_status_code(status);
        for h in resp_headers {
            r = r.with_header(h);
        }
        let _ = req.respond(r);
    }
}
