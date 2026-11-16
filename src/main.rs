//! sinterklaas — load config, connect and migrate the database, make sure the
//! Sint exists, sweep expired links and sessions hourly, serve.

use anyhow::Result;
use sinterklaas::{config::Config, db, routes, state::AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;
    db::ensure_admin(&pool, &cfg.admin_email, &cfg.admin_name).await?;

    let sweep_pool = pool.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match db::sweep(&sweep_pool).await {
                Ok(n) if n > 0 => tracing::info!("swept {n} expired link(s) and session(s)"),
                Ok(_) => {}
                Err(e) => tracing::warn!("sweep failed: {e:#}"),
            }
        }
    });

    let bind_addr = cfg.bind_addr.clone();
    let app = routes::router(AppState::new(pool, cfg));
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("sinterklaas listening on {bind_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
