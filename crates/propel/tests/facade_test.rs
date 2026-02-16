use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use propel::auth::{PropelAuth, SupabaseClaims};
use propel::state::PropelState;
use tower::ServiceExt;

const TEST_SECRET: &str = "test-jwt-secret-at-least-32-chars-long";

fn test_state() -> PropelState {
    PropelState {
        supabase_url: "https://test.supabase.co".to_owned(),
        supabase_anon_key: "anon-key".to_owned(),
        supabase_jwt_secret: TEST_SECRET.to_owned(),
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

fn build_app(state: PropelState) -> Router {
    Router::new()
        .route("/protected", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            PropelAuth::verify,
        ))
        .with_state(state)
}

// ── Normal cases ──

#[tokio::test]
async fn valid_token_passes_through() {
    let state = test_state();
    let app = build_app(state);

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
async fn claims_attached_to_extensions() {
    let state = test_state();
    let claims = valid_claims();
    let token = make_token(&claims, TEST_SECRET);

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

// ── Error cases ──

#[tokio::test]
async fn missing_auth_header_returns_401() {
    let state = test_state();
    let app = build_app(state);

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
    let state = test_state();
    let app = build_app(state);

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
    let state = test_state();
    let app = build_app(state);

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
    let state = test_state();
    let app = build_app(state);

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
    let state = test_state();
    let app = build_app(state);

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
    let state = test_state();
    let app = build_app(state);

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
