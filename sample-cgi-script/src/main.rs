use axum::{routing::get, Router};
use rusqlite::Connection;
use tower_cgi::serve_cgi;
use tower_cookies::{Cookie, Cookies};
use tower_sessions::cookie::time::Duration;
use tower_sessions::{MemoryStore, Session};

#[tokio::main]
async fn main() {
    // tower-sessions-file-based-store pulls axum 0.7; MemoryStore shares our axum 0.8 tree.
    let session_store = MemoryStore::default();
    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_secure(false)
        //.with_always_save(true)
        .with_expiry(tower_sessions::Expiry::OnInactivity(Duration::seconds(15)));

    let app = Router::new()
        .route(
            "/cgi-bin/sample-cgi-server/",
            get(|cookies: Cookies, session: Session| async move {
                cookies.add(Cookie::new("hello_world", "hello_world"));
                session.clear().await;
                session.insert("foo", "bar").await.unwrap();
                let _value: String = session
                    .get("foo")
                    .await
                    .unwrap()
                    .unwrap_or("no value".to_string());

                let conn = Connection::open("./my_database.db").unwrap();
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS user (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            age INTEGER
        )",
                    [],
                )
                .unwrap();

                let mut stmt = conn
                    .prepare("INSERT INTO user (name, age) VALUES (?,?)")
                    .unwrap();
                stmt.execute(["Alice", "30"]).unwrap();

                let mut stmt = conn.prepare("SELECT * FROM user").unwrap();
                let mut one: String = "".into();
                let mut rows = stmt.query([]).unwrap();
                while let Some(row) = rows.next().unwrap() {
                    let name: String = row.get(1).unwrap();
                    one.push_str(name.as_str())
                }

                one
            }),
        )
        .route(
            "/cgi-bin/sample-cgi-server/with/path-info",
            get(|| async { "Hello, PATH_INFO" }),
        )
        .layer(session_layer);

    if let Err(e) = serve_cgi(app).await {
        eprintln!("Error while serving CGI request: {}", e);
        std::process::exit(1);
    }
}
