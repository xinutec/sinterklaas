//! Everything the site keeps: the people, their links and sessions, the draw.

use anyhow::{Context, Result, bail};
use chrono::{Duration, NaiveDateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

pub async fn connect(url: &str) -> Result<MySqlPool> {
    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .context("connecting to the database")
}

pub async fn migrate(pool: &MySqlPool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Person {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub wishlist: String,
    pub is_admin: bool,
    pub invited_at: Option<NaiveDateTime>,
}

/// `SELECT … FROM person ` followed by `$tail`, as one literal: sqlx takes no
/// SQL built at run time.
macro_rules! select_person {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, email, wishlist, is_admin, invited_at FROM person ",
            $tail
        )
    };
}

/// Lower-cased and trimmed: the address someone types to sign in must find the
/// one the Sint typed to invite them.
pub fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// The Sint, made admin on every start so the first invite needs no database
/// edit. The Sint keeps the book: no wish list, not in the draw.
pub async fn ensure_admin(pool: &MySqlPool, email: &str, name: &str) -> Result<()> {
    let at = now();
    sqlx::query(
        "INSERT INTO person (name, email, wishlist, is_admin, created_at, updated_at) \
         VALUES (?, ?, '', TRUE, ?, ?) ON DUPLICATE KEY UPDATE is_admin = TRUE",
    )
    .bind(name)
    .bind(normalise_email(email))
    .bind(at)
    .bind(at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn person(pool: &MySqlPool, id: u64) -> Result<Option<Person>> {
    Ok(sqlx::query_as(select_person!("WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

pub async fn person_by_email(pool: &MySqlPool, email: &str) -> Result<Option<Person>> {
    Ok(sqlx::query_as(select_person!("WHERE email = ?"))
        .bind(normalise_email(email))
        .fetch_optional(pool)
        .await?)
}

/// Everyone taking part: the Sint keeps the book and is not in the draw.
pub async fn participants(pool: &MySqlPool) -> Result<Vec<Person>> {
    Ok(
        sqlx::query_as(select_person!("WHERE is_admin = FALSE ORDER BY name, id"))
            .fetch_all(pool)
            .await?,
    )
}

/// `false` when the address is already invited.
pub async fn add_person(pool: &MySqlPool, name: &str, email: &str) -> Result<bool> {
    let at = now();
    let done = sqlx::query(
        "INSERT IGNORE INTO person (name, email, wishlist, is_admin, created_at, updated_at) \
         VALUES (?, ?, '', FALSE, ?, ?)",
    )
    .bind(name)
    .bind(normalise_email(email))
    .bind(at)
    .bind(at)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Refused once the draw has been made: removing someone then would leave
/// a giver with nobody to buy for, or a person nobody buys for.
pub async fn remove_person(pool: &MySqlPool, id: u64) -> Result<()> {
    if drawn(pool).await? {
        bail!("the draw has been made");
    }
    sqlx::query("DELETE FROM person WHERE id = ? AND is_admin = FALSE")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_wishes(pool: &MySqlPool, id: u64, name: &str, wishlist: &str) -> Result<()> {
    sqlx::query("UPDATE person SET name = ?, wishlist = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(wishlist)
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_invited(pool: &MySqlPool, id: u64) -> Result<()> {
    sqlx::query("UPDATE person SET invited_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// --- sign-in links -----------------------------------------------------------

/// When `person` was last sent a link, for the one-a-minute limit.
pub async fn last_link_at(pool: &MySqlPool, person: u64) -> Result<Option<NaiveDateTime>> {
    let row: Option<(NaiveDateTime,)> =
        sqlx::query_as("SELECT MAX(created_at) FROM login_link WHERE person_id = ? HAVING MAX(created_at) IS NOT NULL")
            .bind(person)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(at,)| at))
}

pub async fn create_link(
    pool: &MySqlPool,
    person: u64,
    token_hash: &str,
    valid: Duration,
) -> Result<()> {
    let at = now();
    sqlx::query(
        "INSERT INTO login_link (token_hash, person_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(person)
    .bind(at)
    .bind(at + valid)
    .execute(pool)
    .await?;
    Ok(())
}

/// Spends the link: whose it was, if it was unused and unexpired. One
/// statement, so two clicks cannot both succeed.
pub async fn use_link(pool: &MySqlPool, token_hash: &str) -> Result<Option<u64>> {
    let at = now();
    let spent = sqlx::query(
        "UPDATE login_link SET used_at = ? WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?",
    )
    .bind(at)
    .bind(token_hash)
    .bind(at)
    .execute(pool)
    .await?;
    if spent.rows_affected() != 1 {
        return Ok(None);
    }
    let row: Option<(u64,)> =
        sqlx::query_as("SELECT person_id FROM login_link WHERE token_hash = ?")
            .bind(token_hash)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id,)| id))
}

// --- sessions -----------------------------------------------------------------

pub async fn create_session(
    pool: &MySqlPool,
    person: u64,
    token_hash: &str,
    valid: Duration,
) -> Result<()> {
    sqlx::query("INSERT INTO session (token_hash, person_id, expires_at) VALUES (?, ?, ?)")
        .bind(token_hash)
        .bind(person)
        .bind(now() + valid)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn session_person(pool: &MySqlPool, token_hash: &str) -> Result<Option<Person>> {
    Ok(sqlx::query_as(
        "SELECT p.id, p.name, p.email, p.wishlist, p.is_admin, p.invited_at \
         FROM session s JOIN person p ON p.id = s.person_id \
         WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(token_hash)
    .bind(now())
    .fetch_optional(pool)
    .await?)
}

pub async fn end_session(pool: &MySqlPool, token_hash: &str) -> Result<()> {
    sqlx::query("DELETE FROM session WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Expired links and sessions: kept for nothing once they cannot be used.
pub async fn sweep(pool: &MySqlPool) -> Result<u64> {
    let at = now();
    let links = sqlx::query("DELETE FROM login_link WHERE expires_at < ?")
        .bind(at - Duration::days(1))
        .execute(pool)
        .await?;
    let sessions = sqlx::query("DELETE FROM session WHERE expires_at < ?")
        .bind(at)
        .execute(pool)
        .await?;
    Ok(links.rows_affected() + sessions.rows_affected())
}

// --- the draw -----------------------------------------------------------------

pub async fn drawn(pool: &MySqlPool) -> Result<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM draw")
        .fetch_one(pool)
        .await?;
    Ok(n > 0)
}

/// Records the draw over every participant, in one transaction. Refused when one has
/// already been made: a second draw would change who buys for whom after
/// people have started shopping.
pub async fn make_draw(pool: &MySqlPool) -> Result<usize> {
    let mut tx = pool.begin().await?;
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM draw FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if n > 0 {
        bail!("the draw has already been made");
    }
    let ids: Vec<(u64,)> =
        sqlx::query_as("SELECT id FROM person WHERE is_admin = FALSE ORDER BY id FOR UPDATE")
            .fetch_all(&mut *tx)
            .await?;
    let Some(to) = crate::draw::derangement(ids.len(), &mut rand::rng()) else {
        bail!("a draw needs at least two people");
    };
    let at = now();
    for (giver, receiver) in to.iter().enumerate() {
        sqlx::query("INSERT INTO draw (giver_id, receiver_id, drawn_at) VALUES (?, ?, ?)")
            .bind(ids[giver].0)
            .bind(ids[*receiver].0)
            .bind(at)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(ids.len())
}

/// Undoes the draw, for a mistake made before anyone shopped.
pub async fn reset_draw(pool: &MySqlPool) -> Result<()> {
    sqlx::query("DELETE FROM draw").execute(pool).await?;
    Ok(())
}

/// Whom `giver` buys for, once the draw is made.
pub async fn receiver_of(pool: &MySqlPool, giver: u64) -> Result<Option<Person>> {
    Ok(sqlx::query_as(
        "SELECT p.id, p.name, p.email, p.wishlist, p.is_admin, p.invited_at \
         FROM draw d JOIN person p ON p.id = d.receiver_id WHERE d.giver_id = ?",
    )
    .bind(giver)
    .fetch_optional(pool)
    .await?)
}
