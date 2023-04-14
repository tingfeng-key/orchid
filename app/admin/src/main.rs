use axum::{extract::MatchedPath, http::Request};
use error::{ErrorCode, Result};
use std::net::SocketAddr;
use time::{macros::format_description, UtcOffset};
use tracing_subscriber::{fmt::time::OffsetTime, prelude::*, EnvFilter};

mod captcha;
mod ctls;
mod error;
mod extracts;
mod jwt;
mod middleware;
mod openapi;
mod password;
mod router;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    //
    let local_time = OffsetTime::new(
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );
    let env_filter = EnvFilter::builder().parse_lossy(format!(
        "{}=INFO,tower_http=debug,axum::rejection=trace",
        env!("CARGO_PKG_NAME")
    ));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_timer(local_time))
        .with(env_filter)
        .init();

    let captcha = captcha::Captcha::new(2, 10 * 60);
    let jwt = jwt::Jwt::new("secret", 2, 7 * 24 * 60);
    let prisma_client = service::new_client().await?;
    let state = state::State::build(captcha, jwt, prisma_client);

    let app = router::init(state).await.layer(
        tower_http::trace::TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
            // Log the matched route's path (with placeholders not filled in).
            // Use request.uri() or OriginalUri if you want the real path.
            let matched_path = request
                .extensions()
                .get::<MatchedPath>()
                .map(MatchedPath::as_str);

            tracing::info_span!(
                "http_request",
                method = ?request.method(),
                matched_path,
                some_other_field = tracing::field::Empty,
            )
        }),
    );
    let server_address = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Service is running on {}", server_address);

    // let (password, salt) = password::Password::generate_hash_salt("123456".as_bytes())?;
    // println!("{}, {}", password, salt);

    axum::Server::bind(&server_address)
        .serve(app.into_make_service())
        .await
        .map_err(|_| ErrorCode::ServerSteup)?;
    Ok(())
}
