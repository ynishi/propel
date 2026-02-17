use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use propel::auth::{AuthIdentity, PropelAuth, SupabaseClaims};
use propel::state::PropelState;
use secrecy::SecretString;
use tower::ServiceExt;

const TEST_SECRET: &str = "test-jwt-secret-at-least-32-chars-long";
const TEST_SERVER_KEY: &str = "test-server-key-at-least-32-chars-long";

fn test_state() -> PropelState {
    PropelState {
        supabase_url: "https://test.supabase.co".to_owned(),
        supabase_anon_key: SecretString::from("anon-key".to_owned()),
        supabase_jwt_secret: SecretString::from(TEST_SECRET.to_owned()),
        server_key: Some(SecretString::from(TEST_SERVER_KEY.to_owned())),
    }
}

fn test_state_no_server_key() -> PropelState {
    PropelState {
        supabase_url: "https://test.supabase.co".to_owned(),
        supabase_anon_key: SecretString::from("anon-key".to_owned()),
        supabase_jwt_secret: SecretString::from(TEST_SECRET.to_owned()),
        server_key: None,
    }
}

fn make_token(claims: &SupabaseClaims, secret: &str) -> String {
    let key = EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&Header::new(Algorithm::HS256), claims, &key).unwrap()
}

fn valid_claims() -> SupabaseClaims {
    let now = jsonwebtoken::get_current_timestamp() as usize;
    SupabaseClaims {
        sub: "user-123".to_owned(),
        email: Some("test@example.com".to_owned()),
        role: Some("authenticated".to_owned()),
        iat: now,
        exp: now + 3600,
        aud: "authenticated".to_owned(),
    }
}

fn service_role_claims() -> SupabaseClaims {
    let now = jsonwebtoken::get_current_timestamp() as usize;
    SupabaseClaims {
        sub: "service".to_owned(),
        email: None,
        role: Some("service_role".to_owned()),
        iat: now,
        exp: now + 3600,
        aud: "authenticated".to_owned(),
    }
}

fn build_app(state: PropelState) -> Router {
    Router::new()
        .route("/protected", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state)
}

// ── Normal cases: User JWT ──

#[tokio::test]
async fn valid_user_token_passes_through() {
    let app = build_app(test_state());
    let token = make_token(&valid_claims(), TEST_SECRET);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn user_jwt_attaches_auth_identity_user() {
    let state = test_state();
    let token = make_token(&valid_claims(), TEST_SECRET);

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let identity = req.extensions().get::<AuthIdentity>().unwrap();
                match identity {
                    AuthIdentity::User(c) => format!("user:{}", c.sub),
                    _ => "wrong_variant".to_owned(),
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"user:user-123");
}

#[tokio::test]
async fn supabase_claims_backward_compat() {
    let state = test_state();
    let token = make_token(&valid_claims(), TEST_SECRET);

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let claims = req.extensions().get::<SupabaseClaims>().unwrap();
                claims.sub.clone()
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"user-123");
}

// ── Normal cases: service_role JWT ──

#[tokio::test]
async fn service_role_jwt_passes_through() {
    let app = build_app(test_state());
    let token = make_token(&service_role_claims(), TEST_SECRET);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn service_role_jwt_attaches_auth_identity_service_role() {
    let state = test_state();
    let token = make_token(&service_role_claims(), TEST_SECRET);

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let identity = req.extensions().get::<AuthIdentity>().unwrap();
                match identity {
                    AuthIdentity::ServiceRole(c) => format!("service:{}", c.sub),
                    _ => "wrong_variant".to_owned(),
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"service:service");
}

// ── Normal cases: Server Key ──

#[tokio::test]
async fn valid_server_key_passes_through() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", TEST_SERVER_KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn server_key_attaches_auth_identity_server_key() {
    let state = test_state();

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let identity = req.extensions().get::<AuthIdentity>().unwrap();
                match identity {
                    AuthIdentity::ServerKey => "server_key".to_owned(),
                    _ => "wrong_variant".to_owned(),
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", TEST_SERVER_KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"server_key");
}

#[tokio::test]
async fn server_key_does_not_attach_supabase_claims() {
    let state = test_state();

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let has_claims = req.extensions().get::<SupabaseClaims>().is_some();
                format!("{has_claims}")
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", TEST_SERVER_KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"false");
}

#[tokio::test]
async fn server_key_takes_priority_over_jwt() {
    let state = test_state();
    let token = make_token(&valid_claims(), TEST_SECRET);

    let app = Router::new()
        .route(
            "/protected",
            get(|req: Request<Body>| async move {
                let identity = req.extensions().get::<AuthIdentity>().unwrap();
                match identity {
                    AuthIdentity::ServerKey => "server_key".to_owned(),
                    AuthIdentity::User(_) => "user".to_owned(),
                    AuthIdentity::ServiceRole(_) => "service_role".to_owned(),
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state);

    // Send BOTH X-Server-Key and Authorization headers
    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", TEST_SERVER_KEY)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"server_key");
}

// ── Error cases: JWT ──

#[tokio::test]
async fn missing_auth_header_returns_401() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn missing_bearer_prefix_returns_401() {
    let app = build_app(test_state());
    let token = make_token(&valid_claims(), TEST_SECRET);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", token) // no "Bearer " prefix
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_signature_returns_401() {
    let app = build_app(test_state());
    let token = make_token(&valid_claims(), "wrong-secret-that-is-long-enough!");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_token_returns_401() {
    let app = build_app(test_state());

    let mut claims = valid_claims();
    claims.exp = 1000; // expired long ago
    claims.iat = 900;
    let token = make_token(&claims, TEST_SECRET);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_token_returns_401() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", "Bearer not.a.valid.jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_bearer_returns_401() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", "Bearer ")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ── Error cases: Server Key ──

#[tokio::test]
async fn invalid_server_key_returns_401() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", "wrong-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn server_key_when_not_configured_returns_401() {
    let app = build_app(test_state_no_server_key());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", TEST_SERVER_KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_server_key_header_returns_401() {
    let app = build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("x-server-key", "")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
