use anyhow::anyhow;
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

const ANILIST_AUTH_URL: &str = "https://anilist.co/api/v2/oauth/authorize";
const ANILIST_TOKEN_URL: &str = "https://anilist.co/api/v2/oauth/token";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Start the OAuth flow: open browser and wait for the redirect callback.
/// Returns the access token on success.
pub async fn start_oauth_flow(client_id: &str, client_secret: &str) -> anyhow::Result<String> {
    // Bind to a random available port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{}", port);

    // Build auth URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code",
        ANILIST_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
    );

    // Open browser
    open_browser(&auth_url);

    // Wait for the callback (2 minute timeout)
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_callback(listener),
    )
    .await
    .map_err(|_| anyhow!("OAuth timed out after 2 minutes"))??;

    // Exchange code for token
    let client = reqwest::Client::new();
    let resp: TokenResponse = client
        .post(ANILIST_TOKEN_URL)
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": client_secret,
            "redirect_uri": redirect_uri,
            "code": code,
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(resp.access_token)
}

fn open_browser(url: &str) {
    // Try `open` crate first, fall back to cmd /c start on Windows
    if open::that(url).is_err() {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn();
    }
}

async fn wait_for_callback(listener: TcpListener) -> anyhow::Result<String> {
    let (stream, _) = listener.accept().await?;
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    // Parse GET /?code=XXXXX HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("invalid HTTP request"))?;

    let code = parse_query_param(path, "code")
        .ok_or_else(|| anyhow!("no code in callback: {}", path))?;

    // Send a minimal response so the browser doesn't hang
    // The stream is wrapped in reader; extract it to write back
    use tokio::io::AsyncWriteExt;
    let response_bytes = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAuthorization received. You may close this window.";
    let mut stream = reader.into_inner();
    let _ = stream.write_all(response_bytes).await;

    Ok(code)
}

fn parse_query_param(path: &str, key: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?;
        let v = parts.next().unwrap_or("");
        if k == key {
            return Some(urlencoding::decode(v).ok()?.to_string());
        }
    }
    None
}
