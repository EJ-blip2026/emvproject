use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::{get, post}, Router, Json};
use dashmap::DashMap;
use serde_json::Value;
use std::{collections::HashSet, env, fs, net::SocketAddr, path::Path, sync::Arc, time::Instant};
use tokio::sync::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection as RedisConnection;
use sqlx::AnyPool;
use reqwest::Client;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use axum::body::Bytes;
use axum::http::HeaderMap;
use uuid::Uuid;
use chrono::Utc;
use axum::response::Html;
use sqlx::Row;
use axum::routing::put;
use axum::Json as AxumJson;
use serde_json::json;

#[derive(Clone)]
struct AppState {
    haikus: Arc<Value>,
    // DB-backed keys (AnyPool) supporting Postgres or SQLite
    db_pool: Arc<AnyPool>,
    // in-memory keys cache (fallback / seed)
    api_keys: Arc<RwLock<HashSet<String>>>,
    // per-key in-memory rate state (fallback)
    rate_map: Arc<DashMap<String, Arc<Mutex<(Instant, u32)>>>>,
    // optional Redis async connection for rate-limiting (multiplexed)
    redis_conn: Option<Arc<Mutex<RedisConnection>>>,
    limit: u32,
    window_secs: u64,
    admin_token: String,
    keys_path: String,
}

async fn haikus_handler(State(state): State<AppState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // auth
    let key = match headers.get("x-api-key") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return (StatusCode::UNAUTHORIZED, "invalid api key header").into_response(),
        },
        None => return (StatusCode::UNAUTHORIZED, "missing api key").into_response(),
    };

    // Check key existence in DB first (non-blocking to caller via spawn_blocking)
    // check DB (sqlx AnyPool)
    let key_exists = match sqlx::query_scalar::<_, i64>("SELECT 1 FROM api_keys WHERE key = ?")
        .bind(&key)
        .fetch_optional(&*state.db_pool)
        .await
    {
        Ok(opt) => opt.is_some(),
        Err(_) => false,
    };

    if !key_exists {
        // final fallback: check in-memory cache
        let read_keys = state.api_keys.read().await;
        if !read_keys.contains(&key) {
            return (StatusCode::UNAUTHORIZED, "invalid api key").into_response();
        }
    }

    // rate limiting
    // Rate limiting: prefer Redis if configured, else in-memory
    if let Some(redis_mgr) = &state.redis_conn {
        let mut conn = redis_mgr.lock().await;
        // INCR the key, set EXPIRE if newly created
        let redis_key = format!("rl:{}", key);
        let val: i64 = match conn.incr(&redis_key, 1i64).await {
            Ok(v) => v,
            Err(_) => {
                // on redis error, fall back to in-memory below
                -1
            }
        };
        if val == 1 {
            let _ : redis::RedisResult<()> = conn.expire(&redis_key, state.window_secs as i64).await;
        }
        if val > 0 {
            if val as u32 > state.limit {
                return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
            }
        } else {
            // redis failed, fall back to in-memory
            let entry = state.rate_map.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new((Instant::now(), 0))));
            let lock = entry.value().clone();
            let mut guard = lock.lock().await;
            let now = Instant::now();
            if now.duration_since(guard.0).as_secs() >= state.window_secs {
                guard.0 = now;
                guard.1 = 1;
            } else {
                if guard.1 >= state.limit {
                    return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
                }
                guard.1 += 1;
            }
        }
    } else {
        // in-memory limiter
        let entry = state.rate_map.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new((Instant::now(), 0))));
        let lock = entry.value().clone();
        {
            let mut guard = lock.lock().await;
            let now = Instant::now();
            if now.duration_since(guard.0).as_secs() >= state.window_secs {
                guard.0 = now;
                guard.1 = 1;
            } else {
                if guard.1 >= state.limit {
                    return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
                }
                guard.1 += 1;
            }
        }
    }

    // return haikus.json content
    (StatusCode::OK, [("content-type", "application/json")], state.haikus.to_string()).into_response()
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    price_id: String,
    success_url: Option<String>,
    cancel_url: Option<String>,
    customer_email: Option<String>,
}

async fn create_checkout_handler(State(_state): State<AppState>, Json(payload): Json<CreateSessionRequest>) -> impl IntoResponse {
    let stripe_key = match env::var("STRIPE_API_KEY") {
        Ok(s) => s,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "missing STRIPE_API_KEY").into_response(),
    };

    let client = Client::new();
    let price = payload.price_id;
    let success = payload.success_url.unwrap_or_else(|| "http://localhost:3000/thanks".to_string());
    let cancel = payload.cancel_url.unwrap_or_else(|| "http://localhost:3000/cancel".to_string());

    // build form data expected by Stripe
    // build form data expected by Stripe
    let mut params = vec![
        ("mode", "subscription"),
        ("line_items[0][price]", price.as_str()),
        ("line_items[0][quantity]", "1"),
        ("success_url", success.as_str()),
        ("cancel_url", cancel.as_str()),
    ];
    if let Some(email) = payload.customer_email.as_ref() {
        params.push(("customer_email", email.as_str()));
        // also add metadata so webhook can see email
        params.push(("metadata[email]", email.as_str()));
    }

    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .bearer_auth(stripe_key)
        .form(&params)
        .send()
        .await;

    match res {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(json) => (StatusCode::OK, Json(json)).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "invalid stripe response").into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("request failed: {}", e)).into_response(),
    }
}

async fn stripe_webhook_handler(State(_state): State<AppState>, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let secret = match env::var("STRIPE_WEBHOOK_SECRET") {
        Ok(s) => s,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "missing STRIPE_WEBHOOK_SECRET").into_response(),
    };

    let sig = match headers.get("Stripe-Signature") {
        Some(v) => match v.to_str() { Ok(s) => s.to_string(), Err(_) => return (StatusCode::BAD_REQUEST, "invalid signature header").into_response(), },
        None => return (StatusCode::BAD_REQUEST, "missing signature").into_response(),
    };

    // parse signature header for t and v1
    let mut timestamp: Option<String> = None;
    let mut v1: Option<String> = None;
    for part in sig.split(',') {
        let mut kv = part.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            let k = k.trim();
            let v = v.trim();
            if k == "t" { timestamp = Some(v.to_string()); }
            if k == "v1" { v1 = Some(v.to_string()); }
        }
    }

    let timestamp = match timestamp { Some(t) => t, None => return (StatusCode::BAD_REQUEST, "missing t").into_response(), };
    let v1 = match v1 { Some(s) => s, None => return (StatusCode::BAD_REQUEST, "missing v1").into_response(), };

    // payload is `{timestamp}.{raw_body}`
    let payload = format!("{}.{}", timestamp, String::from_utf8_lossy(&body));

    // verify signature
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) { Ok(m) => m, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "hmac init failed").into_response(), };
    mac.update(payload.as_bytes());
    let sig_bytes = match hex::decode(v1) { Ok(b) => b, Err(_) => return (StatusCode::BAD_REQUEST, "invalid v1 hex").into_response(), };
    if mac.verify_slice(&sig_bytes).is_err() {
        return (StatusCode::UNAUTHORIZED, "signature mismatch").into_response();
    }

    // parse body JSON
    let json: serde_json::Value = match serde_json::from_slice(&body) { Ok(j) => j, Err(_) => return (StatusCode::BAD_REQUEST, "invalid json").into_response(), };

    // idempotency: check if event already processed
    let event_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if event_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "missing event id").into_response();
    }

    let pool = &* _state.db_pool;
    let already = sqlx::query_scalar::<_, String>("SELECT id FROM webhook_events WHERE id = ?")
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .is_some();
    if already {
        return (StatusCode::OK, "already processed").into_response();
    }

    // persist raw event for audit
    let now = Utc::now().to_rfc3339();
    let _ = sqlx::query("INSERT INTO webhook_events (id, event_type, payload, received_at) VALUES (?, ?, ?, ?)")
        .bind(event_id).bind(json.get("type").and_then(|t| t.as_str()).unwrap_or("")).bind(String::from_utf8_lossy(&body).to_string()).bind(&now)
        .execute(pool).await;

    // persist webhook event in `usage` or `subscriptions` depending on event type
    if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
        match event_type {
            "checkout.session.completed" => {
                let obj = &json["data"]["object"];
                // try to extract customer email from metadata or customer_details
                let email = obj.get("metadata").and_then(|m| m.get("email")).and_then(|e| e.as_str())
                    .or_else(|| obj.get("customer_details").and_then(|c| c.get("email")).and_then(|e| e.as_str()));

                let plan = obj.get("metadata").and_then(|m| m.get("plan")).and_then(|p| p.as_str()).unwrap_or("subscription");

                if let Some(email) = email {
                    // create or find user and api_key, persist subscription
                    let user_id = match sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email = ?")
                        .bind(email)
                        .fetch_optional(pool).await
                    {
                        Ok(Some(id)) => id,
                        _ => {
                            // insert new user
                            let new_user = Uuid::new_v4().to_string();
                            let now = Utc::now().to_rfc3339();
                            let _ = sqlx::query("INSERT INTO users (id, email, created_at) VALUES (?, ?, ?)")
                                .bind(&new_user).bind(email).bind(&now).execute(pool).await;
                            new_user
                        }
                    };

                    // create subscription record
                    let sub_id = obj.get("subscription").and_then(|s| s.as_str()).map(|s| s.to_string()).unwrap_or_else(|| Uuid::new_v4().to_string());
                    let now2 = Utc::now().to_rfc3339();
                    let _ = sqlx::query("INSERT OR REPLACE INTO subscriptions (id, user_id, plan, status, current_period_end, created_at) VALUES (?, ?, ?, ?, ?, ?)")
                        .bind(&sub_id).bind(&user_id).bind(plan).bind("active").bind(None::<String>).bind(&now2).execute(pool).await;

                    // create an api_key for this user
                    let api_key = Uuid::new_v4().to_string();
                    let _ = sqlx::query("INSERT OR REPLACE INTO api_keys (key, user_id, created_at, last_rotated) VALUES (?, ?, ?, ?)")
                        .bind(&api_key).bind(&user_id).bind(&now2).bind(&now2).execute(pool).await;

                    // update in-memory cache via RwLock
                    let mut keys = _state.api_keys.write().await;
                    keys.insert(api_key.clone());

                    // persist event into usage table for records
                    let usage_id = Uuid::new_v4().to_string();
                    let _ = sqlx::query("INSERT INTO usage (id, api_key, endpoint, count, window_start) VALUES (?, ?, ?, ?, ?)")
                        .bind(&usage_id).bind(&api_key).bind("/billing/checkout").bind(1i64).bind(&now2).execute(pool).await;
                }
            }
            "invoice.payment_succeeded" => {
                // rotate key on successful payment
                if let Some(obj) = json.get("data").and_then(|d| d.get("object")) {
                    // determine customer email or subscription
                    let customer = obj.get("customer").and_then(|c| c.as_str());
                    // if subscription present, find user_id by subscription
                    if let Some(sub_id) = obj.get("subscription").and_then(|s| s.as_str()) {
                        if let Ok(Some(user_id)) = sqlx::query_scalar::<_, String>("SELECT user_id FROM subscriptions WHERE id = ?")
                            .bind(sub_id).fetch_optional(pool).await
                        {
                            // rotate key for user
                            let new_key = Uuid::new_v4().to_string();
                            let now3 = Utc::now().to_rfc3339();
                            let _ = sqlx::query("INSERT INTO api_keys (key, user_id, created_at, last_rotated) VALUES (?, ?, ?, ?) ")
                                .bind(&new_key).bind(&user_id).bind(&now3).bind(&now3).execute(pool).await;
                            // also could mark old keys as rotated or delete; for now, keep old keys
                        }
                    }
                }
            }
            "invoice.payment_failed" => {
                // mark subscription as past_due if subscription id available
                if let Some(obj) = json.get("data").and_then(|d| d.get("object")) {
                    if let Some(sub_id) = obj.get("subscription").and_then(|s| s.as_str()) {
                        let _ = sqlx::query("UPDATE subscriptions SET status = ? WHERE id = ?")
                            .bind("past_due").bind(sub_id).execute(pool).await;
                    }
                }
            }
            "customer.subscription.updated" | "customer.subscription.deleted" => {
                if let Some(obj) = json.get("data").and_then(|d| d.get("object")) {
                    if let Some(sub_id) = obj.get("id").and_then(|s| s.as_str()) {
                        let status = obj.get("status").and_then(|s| s.as_str()).unwrap_or("unknown");
                        let _ = sqlx::query("UPDATE subscriptions SET status = ? WHERE id = ?")
                            .bind(status).bind(sub_id).execute(pool).await;
                    }
                }
            }
            _ => {}
        }
    }

    (StatusCode::OK, "webhook received").into_response()
}

async fn billing_ui() -> impl IntoResponse {
    match std::fs::read_to_string("public/stripe_checkout.html") {
        Ok(s) => Html(s).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "ui not found").into_response(),
    }
}

async fn pricing_ui() -> impl IntoResponse {
    match std::fs::read_to_string("public/pricing.html") {
        Ok(s) => Html(s).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "pricing not found").into_response(),
    }
}

async fn thanks_ui() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Thank You! - Haiku API</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            text-align: center;
            background: white;
            padding: 60px 40px;
            border-radius: 12px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
            max-width: 500px;
        }
        h1 {
            color: #667eea;
            font-size: 2em;
            margin: 0 0 20px;
        }
        p {
            color: #666;
            font-size: 1.1em;
            line-height: 1.6;
            margin: 20px 0;
        }
        .success-icon {
            font-size: 4em;
            margin-bottom: 20px;
        }
        a {
            display: inline-block;
            background: #667eea;
            color: white;
            padding: 12px 30px;
            border-radius: 6px;
            text-decoration: none;
            margin-top: 30px;
            font-weight: 600;
            transition: background 0.3s;
        }
        a:hover {
            background: #764ba2;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="success-icon">✓</div>
        <h1>Thank You!</h1>
        <p>Your subscription is active and your API key is ready.</p>
        <p>Check your email for your API key and getting started guide.</p>
        <p style="font-size: 0.95em; color: #999; margin-top: 40px;">API Key Format: Your key starts with <code>pk_</code> or a UUID.</p>
        <a href="/pricing.html">View Plans</a>
    </div>
</body>
</html>
    "#;
    Html(html).into_response()
}

async fn health() -> &'static str { "ok" }

#[derive(Deserialize)]
struct NewKey {
    key: String,
}

async fn add_key_handler(State(state): State<AppState>, headers: axum::http::HeaderMap, Json(payload): Json<NewKey>) -> impl IntoResponse {
    // admin auth
    let token = match headers.get("x-admin-token") {
        Some(v) => match v.to_str() { Ok(s) => s, Err(_) => return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response(), },
        None => return (StatusCode::UNAUTHORIZED, "missing admin token").into_response(),
    };
    if token != state.admin_token {
        return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response();
    }

    // add key to in-memory set
    {
        let mut keys = state.api_keys.write().await;
        keys.insert(payload.key.clone());
    }

    // persist to file (rewrite full file)
    if let Ok(v) = fs::read_to_string(&state.keys_path) {
        let mut arr: Vec<String> = serde_json::from_str(&v).unwrap_or_default();
        if !arr.contains(&payload.key) {
            arr.push(payload.key.clone());
            if let Ok(s) = serde_json::to_string_pretty(&arr) {
                let _ = fs::create_dir_all(std::path::Path::new(&state.keys_path).parent().unwrap_or(std::path::Path::new(".")));
                let _ = fs::write(&state.keys_path, s);
            }
        }
    } else {
        // file missing — create
        let arr = vec![payload.key.clone()];
        if let Ok(s) = serde_json::to_string_pretty(&arr) {
            let _ = fs::create_dir_all(std::path::Path::new(&state.keys_path).parent().unwrap_or(std::path::Path::new(".")));
            let _ = fs::write(&state.keys_path, s);
        }
    }

    (StatusCode::CREATED, "key added").into_response()
}

async fn list_keys_handler(State(state): State<AppState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // admin auth
    let token = match headers.get("x-admin-token") {
        Some(v) => match v.to_str() { Ok(s) => s, Err(_) => return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response(), },
        None => return (StatusCode::UNAUTHORIZED, "missing admin token").into_response(),
    };
    if token != state.admin_token {
        return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response();
    }

    let pool = &* state.db_pool;
    let rows = sqlx::query("SELECT key, user_id, created_at, last_rotated FROM api_keys")
        .fetch_all(pool).await;

    match rows {
        Ok(rows) => {
            let mut arr = Vec::new();
            for r in rows {
                let key: Option<String> = r.try_get("key").ok();
                let user_id: Option<String> = r.try_get("user_id").ok();
                let created_at: Option<String> = r.try_get("created_at").ok();
                let last_rotated: Option<String> = r.try_get("last_rotated").ok();
                arr.push(json!({"key": key, "user_id": user_id, "created_at": created_at, "last_rotated": last_rotated}));
            }
            (StatusCode::OK, AxumJson(json!(arr))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {}", e)).into_response(),
    }
}

async fn list_subs_handler(State(state): State<AppState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // admin auth
    let token = match headers.get("x-admin-token") {
        Some(v) => match v.to_str() { Ok(s) => s, Err(_) => return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response(), },
        None => return (StatusCode::UNAUTHORIZED, "missing admin token").into_response(),
    };
    if token != state.admin_token {
        return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response();
    }

    let pool = &* state.db_pool;
    let rows = sqlx::query("SELECT id, user_id, plan, status, current_period_end, created_at FROM subscriptions")
        .fetch_all(pool).await;

    match rows {
        Ok(rows) => {
            let mut arr = Vec::new();
            for r in rows {
                let id: Option<String> = r.try_get("id").ok();
                let user_id: Option<String> = r.try_get("user_id").ok();
                let plan: Option<String> = r.try_get("plan").ok();
                let status: Option<String> = r.try_get("status").ok();
                let current_period_end: Option<String> = r.try_get("current_period_end").ok();
                let created_at: Option<String> = r.try_get("created_at").ok();
                arr.push(json!({"id": id, "user_id": user_id, "plan": plan, "status": status, "current_period_end": current_period_end, "created_at": created_at}));
            }
            (StatusCode::OK, AxumJson(json!(arr))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {}", e)).into_response(),
    }
}

#[derive(Deserialize)]
struct RotateRequest {
    key: Option<String>,
    user_id: Option<String>,
}

async fn rotate_key_handler(State(state): State<AppState>, headers: axum::http::HeaderMap, Json(payload): Json<RotateRequest>) -> impl IntoResponse {
    // admin auth
    let token = match headers.get("x-admin-token") {
        Some(v) => match v.to_str() { Ok(s) => s, Err(_) => return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response(), },
        None => return (StatusCode::UNAUTHORIZED, "missing admin token").into_response(),
    };
    if token != state.admin_token {
        return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response();
    }

    let pool = &* state.db_pool;
    // determine user_id
    let user_id = if let Some(k) = payload.key.as_ref() {
        // look up user_id by key
        match sqlx::query_scalar::<_, String>("SELECT user_id FROM api_keys WHERE key = ?")
            .bind(k).fetch_optional(pool).await
        {
            Ok(Some(uid)) => Some(uid),
            _ => None,
        }
    } else {
        payload.user_id.clone()
    };

    let user_id = match user_id {
        Some(u) => u,
        None => return (StatusCode::BAD_REQUEST, "missing user_id or unknown key").into_response(),
    };

    let new_key = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query("INSERT INTO api_keys (key, user_id, created_at, last_rotated, revoked) VALUES (?, ?, ?, ?, ?)")
        .bind(&new_key).bind(&user_id).bind(&now).bind(&now).bind(0).execute(pool).await;

    if let Err(e) = res {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {}", e)).into_response();
    }

    // If original key provided, mark it as revoked (not deleted)
    if let Some(old_key) = payload.key.as_ref() {
        // Mark old key as revoked
        let _ = sqlx::query("UPDATE api_keys SET revoked = 1 WHERE key = ?")
            .bind(old_key).execute(pool).await;
        
        // Insert audit log
        let rotation_id = Uuid::new_v4().to_string();
        let admin_token_masked = token.chars().take(4).chain("****".chars()).collect::<String>();
        let _ = sqlx::query("INSERT INTO key_rotations (id, user_id, old_key, new_key, admin_token, reason, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&rotation_id).bind(&user_id).bind(old_key).bind(&new_key).bind(&admin_token_masked).bind("admin rotation").bind(&now)
            .execute(pool).await;
        
        // Insert notification (in production, send email or push)
        let notif_id = Uuid::new_v4().to_string();
        let msg = format!("Your API key was rotated at {}. Old key is now revoked.", now);
        let _ = sqlx::query("INSERT INTO notifications (id, user_id, channel, message, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&notif_id).bind(&user_id).bind("email").bind(&msg).bind(&now)
            .execute(pool).await;

        let mut keys = state.api_keys.write().await;
        keys.remove(old_key);
    }

    // add new key to in-memory cache
    {
        let mut keys = state.api_keys.write().await;
        keys.insert(new_key.clone());
    }

    (StatusCode::OK, AxumJson(json!({"new_key": new_key}))).into_response()
}

async fn usage_handler(State(state): State<AppState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // admin auth
    let token = match headers.get("x-admin-token") {
        Some(v) => match v.to_str() { Ok(s) => s, Err(_) => return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response(), },
        None => return (StatusCode::UNAUTHORIZED, "missing admin token").into_response(),
    };
    if token != state.admin_token {
        return (StatusCode::UNAUTHORIZED, "invalid admin token").into_response();
    }

    let mut map = serde_json::Map::new();
    for item in state.rate_map.iter() {
        let key = item.key().clone();
        let m = item.value().clone();
        let guard = m.lock().await;
        map.insert(key.clone(), serde_json::json!({"seconds_since_window_start": guard.0.elapsed().as_secs(), "count": guard.1}));
    }

    (StatusCode::OK, Json(serde_json::Value::Object(map))).into_response()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Embed haikus.json at compile time to avoid runtime file-missing crashes; allow env override if provided
    const EMBEDDED_HAIKUS: &str = include_str!("../assets/haikus.json");
    let haikus_raw = match env::var("HAIKUS_PATH") {
        Ok(path) => fs::read_to_string(&path).unwrap_or_else(|_| EMBEDDED_HAIKUS.to_string()),
        Err(_) => EMBEDDED_HAIKUS.to_string(),
    };
    let haikus: Value = serde_json::from_str(&haikus_raw).expect("invalid json");

    // Register compiled sqlx-any drivers (sqlite/postgres) before connecting
    sqlx::any::install_default_drivers();

    let keys_path = env::var("KEYS_PATH").unwrap_or_else(|_| "data/api_keys.json".to_string());
    let keys_db = env::var("KEYS_DB").unwrap_or_else(|_| "data/keys.db".to_string());
    let admin_token = env::var("ADMIN_TOKEN").unwrap_or_else(|_| "admintoken".to_string());

    // ensure DB dir exists
    if let Some(parent) = Path::new(&keys_db).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // initialize sqlx AnyPool (Postgres if DATABASE_URL set, otherwise sqlite file creds)
    let mut database_url = env::var("DATABASE_URL").unwrap_or_else(|_| format!("sqlite://{}", keys_db));

    // CLI arg: support `migrate` to run DB migrations and exit
    let args: Vec<String> = env::args().collect();
    let do_migrate = args.iter().any(|a| a == "migrate" || a == "--migrate" || a == "-m");

    let pool = match AnyPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(err) => {
            // If we're using sqlite and the file cannot be opened (common on read-only hosts), fall back to in-memory sqlite.
            if database_url.starts_with("sqlite://") {
                eprintln!("Failed to open sqlite database '{}': {err}. Falling back to in-memory sqlite.", database_url);
                database_url = "sqlite::memory:".to_string();
                AnyPool::connect(&database_url).await.expect("failed to connect to in-memory sqlite")
            } else {
                panic!("failed to connect to database: {err}");
            }
        }
    };

    println!("Applying migrations against {}", database_url);
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrations failed");
    if do_migrate {
        println!("Migrations applied (explicit migrate requested). Continuing startup...");
    } else {
        println!("Migrations applied.");
    }

    // Load initial keys from DB if present, otherwise fall back to env
    let rows: Vec<String> = sqlx::query_scalar("SELECT key FROM api_keys")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    let mut api_keys: HashSet<String> = rows.into_iter().collect();
    if api_keys.is_empty() {
        let keys_env = env::var("API_KEYS").unwrap_or_else(|_| "testkey".to_string());
        api_keys = keys_env.split(',').map(|s| s.trim().to_string()).collect();
    }

    let limit = env::var("RATE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(60);
    let window_secs = env::var("RATE_WINDOW_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60);

    // optional Redis (use multiplexed connection for production)
    let redis_conn = match env::var("REDIS_URL") {
        Ok(url) => match redis::Client::open(url.as_str()) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(c) => Some(Arc::new(Mutex::new(c))),
                Err(_) => None,
            },
            Err(_) => None,
        },
        Err(_) => None,
    };

    let state = AppState {
        haikus: Arc::new(haikus),
        db_pool: Arc::new(pool),
        api_keys: Arc::new(RwLock::new(api_keys)),
        rate_map: Arc::new(DashMap::new()),
        redis_conn,
        limit,
        window_secs,
        admin_token,
        keys_path,
    };

    let app = Router::new()
        .route("/api/haikus", get(haikus_handler))
        .route("/health", get(health))
        .route("/admin/keys", post(add_key_handler))
        .route("/admin/usage", get(usage_handler))
        .route("/admin/list-keys", get(list_keys_handler))
        .route("/admin/list-subs", get(list_subs_handler))
        .route("/admin/rotate-key", post(rotate_key_handler))
        .route("/billing/create-checkout-session", post(create_checkout_handler))
        .route("/billing/webhook", post(stripe_webhook_handler))
        .route("/billing/ui", get(billing_ui))
        .route("/pricing.html", get(pricing_ui))
        .route("/pricing", get(pricing_ui))
        .route("/thanks.html", get(thanks_ui))
        .route("/thanks", get(thanks_ui))
        .with_state(state);

    let port = env::var("PORT").ok().and_then(|s| s.parse::<u16>().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
