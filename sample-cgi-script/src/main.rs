use axum::{routing::get, Router};
use tower_cookies::{Cookie, Cookies};
use tower_sessions::cookie::time::Duration;
use tower_sessions::{MemoryStore, Session};
use tower_cgi::serve_cgi;

#[tokio::main]
async fn main() {
    // tower-sessions-file-based-store pulls axum 0.7; MemoryStore shares our axum 0.8 tree.
    let session_store = MemoryStore::default();
    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_secure(false)
        //.with_always_save(true)
        .with_expiry(tower_sessions::Expiry::OnInactivity(Duration::seconds(15)));

    let app = Router::new().route(
        "/cgi-bin/sample-cgi-server/",
        get(|cookies: Cookies, session: Session| async move  {
            cookies.add(Cookie::new("hello_world", "hello_world"));
            session.clear().await;
            session.insert("foo", "bar").await.unwrap();
            let value: String = session.get("foo").await.unwrap().unwrap_or("no value".to_string());

            value

        }),
    ).route(
        "/cgi-bin/sample-cgi-server/with/path-info",
        get(|| async { "Hello, PATH_INFO" }),
    ).layer(session_layer);

    if let Err(e) = serve_cgi(app).await {
        eprintln!("Error while serving CGI request: {}", e);
        std::process::exit(1);
    }
}
