use anyhow::anyhow;
use rand::Rng;
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

const ANILIST_AUTH_URL: &str = "https://anilist.co/api/v2/oauth/authorize";
const ANILIST_TOKEN_URL: &str = "https://anilist.co/api/v2/oauth/token";
const OAUTH_PORT: u16 = 35789;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Generate an unpredictable nonce for the OAuth `state` parameter, used to
/// verify the callback came from the authorization request we made (not an
/// attacker racing our fixed loopback port).
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            CHARS[rng.gen_range(0..CHARS.len())] as char
        })
        .collect()
}

/// Start the OAuth flow: open browser and wait for the redirect callback.
/// Returns the access token on success.
pub async fn start_oauth_flow(client_id: &str, client_secret: &str) -> anyhow::Result<String> {
    // Bind to fixed port for registered redirect_uri
    let addr = format!("127.0.0.1:{}", OAUTH_PORT);
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| anyhow!("Port {} is in use. Close other AniVault instances or change OAUTH_PORT. Error: {}", OAUTH_PORT, e))?;
    let redirect_uri = format!("http://{}", addr);
    let expected_state = generate_state();

    // Build auth URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&state={}",
        ANILIST_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&expected_state),
    );

    // Open browser
    open_browser(&auth_url);

    // Wait for the callback (2 minute timeout). Keeps accepting connections
    // until one presents the matching `state`, so a stray/racing local
    // connection can't hijack the flow.
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_matching_callback(listener, &expected_state),
    )
    .await
    .map_err(|_| anyhow!("OAuth timed out after 2 minutes"))??;

    // Exchange code for token
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
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

/// Accept connections until one carries `code` and a `state` matching
/// `expected_state`; anything else is rejected and we keep listening.
async fn wait_for_matching_callback(
    listener: TcpListener,
    expected_state: &str,
) -> anyhow::Result<String> {
    loop {
        let (stream, _) = listener.accept().await?;
        match handle_callback_connection(stream, expected_state).await? {
            Some(code) => return Ok(code),
            None => continue,
        }
    }
}

/// Handle a single connection: parse the callback request, and return the
/// code only if `state` matches. Returns `Ok(None)` for a non-matching or
/// malformed request so the caller keeps waiting for the real callback.
async fn handle_callback_connection(
    stream: tokio::net::TcpStream,
    expected_state: &str,
) -> anyhow::Result<Option<String>> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let path = match request_line.split_whitespace().nth(1) {
        Some(p) => p,
        None => return Ok(None),
    };

    let code = parse_query_param(path, "code");
    let state = parse_query_param(path, "state");

    use tokio::io::AsyncWriteExt;
    let matched = code.is_some() && state.as_deref() == Some(expected_state);
    let (status_line, body) = if matched {
        ("200 OK", "<h1>Connected!</h1><p>AniVault has received your authorization. You may close this window.</p>")
    } else {
        ("400 Bad Request", "<h1>Invalid request</h1><p>This request did not match the expected AniVault sign-in. You can close this window.</p>")
    };
    let response_html = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
        <!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>AniVault</title>\
        <style>body{{background:#080a0f;color:#f4f7fb;font-family:system-ui,sans-serif;\
        display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
        .box{{text-align:center;padding:2rem;border:1px solid rgba(143,183,255,0.2);\
        border-radius:16px}}h1{{font-size:1.4rem;color:#8fb7ff}}p{{color:#9aa6b8}}\
        </style></head><body><div class=\"box\">{body}</div></body></html>"
    );
    let mut stream = reader.into_inner();
    let _ = stream.write_all(response_html.as_bytes()).await;

    if matched {
        Ok(code)
    } else {
        Ok(None)
    }
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
