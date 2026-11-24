//! The site's routes. Every change is a form POST, refused unless it comes
//! from the site's own pages.

use axum::extract::{Path, Query, Request, State};
use axum::http::HeaderValue;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use serde::Deserialize;
use tower::Layer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::db::{self, Person};
use crate::state::AppState;
use crate::{html, mail, token};

const COOKIE: &str = "sinterklaas_session";
const SESSION_DAYS: i64 = 60;
const SIGN_IN_LINK_MINUTES: i64 = 30;
const INVITE_LINK_DAYS: i64 = 7;
/// One link a minute per person, so the form cannot be used to flood a mailbox.
const LINK_GAP_SECONDS: i64 = 60;
const NAME_MAX: usize = 80;
const WISHLIST_MAX: usize = 4000;

pub fn router(state: AppState) -> Router {
    // The stylesheet and the script keep their names across builds, so a
    // browser must ask again each time; a cached old copy would be the wrong
    // site for hours.
    let static_dir = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    )
    .layer(ServeDir::new(&state.cfg.static_dir));
    Router::new()
        .route("/", get(home))
        .route("/login", post(login))
        .route("/in/{token}", get(confirm_link).post(use_link))
        .route("/wishes", post(save_wishes))
        .route("/logout", post(logout))
        .route("/admin", get(admin))
        .route("/admin/people", post(add_person))
        .route("/admin/invite/{id}", post(invite))
        .route("/admin/remove/{id}", post(remove_person))
        .route("/admin/draw", post(draw))
        .route("/admin/reset-draw", post(reset_draw))
        .route("/admin/announce", post(announce))
        .route("/healthz", get(|| async { "ok" }))
        .nest_service("/static", static_dir)
        .layer(middleware::from_fn_with_state(state.clone(), same_origin))
        // A page names its reader and their lot: never kept by a cache.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        ))
        .with_state(state)
}

/// Refuses a POST whose `Origin` (or, failing that, `Referer`) is not this
/// site: a page elsewhere cannot submit these forms with someone's cookie.
async fn same_origin(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.method() == Method::POST && !from_here(req.headers(), &state.cfg.public_url) {
        return (
            StatusCode::FORBIDDEN,
            "Dit formulier kwam niet van de Sint.",
        )
            .into_response();
    }
    next.run(req).await
}

pub fn from_here(headers: &HeaderMap, public_url: &str) -> bool {
    let read = |name| headers.get(name).and_then(|v| v.to_str().ok());
    match (read(header::ORIGIN), read(header::REFERER)) {
        (Some(origin), _) => origin == public_url,
        (None, Some(referer)) => {
            referer == public_url || referer.starts_with(&format!("{public_url}/"))
        }
        (None, None) => false,
    }
}

/// A failure the visitor cannot fix: logged with its cause, shown without it.
struct Failed(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for Failed {
    fn from(e: E) -> Self {
        Failed(e.into())
    }
}

impl IntoResponse for Failed {
    fn into_response(self) -> Response {
        tracing::error!("{:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Er ging iets mis bij de Sint. Probeer het later nog eens.",
        )
            .into_response()
    }
}

type Page = Result<Response, Failed>;

async fn signed_in(state: &AppState, jar: &CookieJar) -> Result<Option<Person>, Failed> {
    match jar.get(COOKIE) {
        None => Ok(None),
        Some(c) => Ok(db::session_person(&state.pool, &token::hash(c.value())).await?),
    }
}

#[derive(Deserialize)]
struct HomeQuery {
    saved: Option<String>,
}

async fn home(State(state): State<AppState>, jar: CookieJar, Query(q): Query<HomeQuery>) -> Page {
    let Some(me) = signed_in(&state, &jar).await? else {
        return Ok(Html(html::sign_in(None, "")).into_response());
    };
    if me.is_admin {
        return Ok(Redirect::to("/admin").into_response());
    }
    let receiver = db::receiver_of(&state.pool, me.id).await?;
    Ok(Html(html::home(&me, receiver.as_ref(), q.saved.is_some())).into_response())
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
}

/// An address not on the list is told so: a mistyped one is noticed, at the
/// price of telling a stranger who takes part.
async fn login(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Page {
    let Some(person) = db::person_by_email(&state.pool, &form.email).await? else {
        let notice = "Dit adres staat niet op de lijst van de Sint. Kijk of het goed gespeld is, of vraag de Sint om een uitnodiging.";
        return Ok(Html(html::sign_in(Some(notice), form.email.trim())).into_response());
    };
    send_link(
        &state,
        &person,
        &mail::Kind::SignIn,
        Duration::minutes(SIGN_IN_LINK_MINUTES),
    )
    .await?;
    Ok(Html(html::check_mail()).into_response())
}

/// Sends `person` a fresh link, unless one went out in the last minute.
async fn send_link(
    state: &AppState,
    person: &Person,
    kind: &mail::Kind,
    valid: Duration,
) -> Result<bool, Failed> {
    if let Some(last) = db::last_link_at(&state.pool, person.id).await?
        && Utc::now().naive_utc() - last < Duration::seconds(LINK_GAP_SECONDS)
    {
        return Ok(false);
    }
    let token = token::new_token();
    db::create_link(&state.pool, person.id, &token::hash(&token), valid).await?;
    let link = format!("{}/in/{token}", state.cfg.public_url);
    mail::send(&state.cfg.mail, &person.name, &person.email, kind, &link).await?;
    Ok(true)
}

async fn confirm_link(Path(token): Path<String>) -> Html<String> {
    Html(html::confirm_link(&token))
}

async fn use_link(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(token): Path<String>,
) -> Page {
    let Some(person) = db::use_link(&state.pool, &token::hash(&token)).await? else {
        return Ok(Html(html::link_spent()).into_response());
    };
    let session = token::new_token();
    db::create_session(
        &state.pool,
        person,
        &token::hash(&session),
        Duration::days(SESSION_DAYS),
    )
    .await?;
    let cookie = Cookie::build((COOKIE, session))
        .path("/")
        .http_only(true)
        .secure(state.cfg.public_url.starts_with("https://"))
        .same_site(SameSite::Lax)
        .max_age(time_days(SESSION_DAYS))
        .build();
    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

fn time_days(days: i64) -> time::Duration {
    time::Duration::days(days)
}

#[derive(Deserialize)]
struct WishesForm {
    name: String,
    wishlist: String,
}

async fn save_wishes(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<WishesForm>,
) -> Page {
    let Some(me) = signed_in(&state, &jar).await?.filter(|p| !p.is_admin) else {
        return Ok(Redirect::to("/").into_response());
    };
    let name = form.name.trim();
    let name = if name.is_empty() {
        me.name.as_str()
    } else {
        name
    };
    let name: String = name.chars().take(NAME_MAX).collect();
    let wishlist: String = form
        .wishlist
        .replace("\r\n", "\n")
        .chars()
        .take(WISHLIST_MAX)
        .collect();
    db::save_wishes(&state.pool, me.id, &name, &wishlist).await?;
    Ok(Redirect::to("/?saved=1").into_response())
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> Page {
    if let Some(c) = jar.get(COOKIE) {
        db::end_session(&state.pool, &token::hash(c.value())).await?;
    }
    Ok((
        jar.remove(Cookie::build(COOKIE).path("/")),
        Redirect::to("/"),
    )
        .into_response())
}

// --- the Sint's pages ------------------------------------------------------------

async fn the_sint(state: &AppState, jar: &CookieJar) -> Result<Option<Person>, Failed> {
    Ok(signed_in(state, jar).await?.filter(|p| p.is_admin))
}

async fn admin_page(state: &AppState, notice: Option<&str>) -> Page {
    let people = db::participants(&state.pool).await?;
    let drawn = db::drawn(&state.pool).await?;
    Ok(Html(html::admin(&people, drawn, notice)).into_response())
}

async fn admin(State(state): State<AppState>, jar: CookieJar) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    admin_page(&state, None).await
}

#[derive(Deserialize)]
struct PersonForm {
    name: String,
    email: String,
}

async fn add_person(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<PersonForm>,
) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    let name: String = form.name.trim().chars().take(NAME_MAX).collect();
    let email = db::normalise_email(&form.email);
    if name.is_empty() || !email.contains('@') {
        return admin_page(&state, Some("Vul een naam en een e-mailadres in.")).await;
    }
    if db::drawn(&state.pool).await? {
        return admin_page(
            &state,
            Some("De lootjes zijn al getrokken: maak de loting eerst ongedaan."),
        )
        .await;
    }
    let notice = if db::add_person(&state.pool, &name, &email).await? {
        format!("{name} staat in het Grote Boek. Vergeet niet uit te nodigen!")
    } else {
        format!("{email} staat er al in.")
    };
    admin_page(&state, Some(&notice)).await
}

async fn invite(State(state): State<AppState>, jar: CookieJar, Path(id): Path<u64>) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    let Some(person) = db::person(&state.pool, id).await? else {
        return admin_page(&state, Some("Die persoon bestaat niet.")).await;
    };
    let sent = send_link(
        &state,
        &person,
        &mail::Kind::Invite,
        Duration::days(INVITE_LINK_DAYS),
    )
    .await?;
    let notice = if sent {
        db::mark_invited(&state.pool, person.id).await?;
        format!("Uitnodiging verstuurd naar {}.", person.name)
    } else {
        format!(
            "{} kreeg net al een link; probeer het over een minuut opnieuw.",
            person.name
        )
    };
    admin_page(&state, Some(&notice)).await
}

async fn remove_person(State(state): State<AppState>, jar: CookieJar, Path(id): Path<u64>) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    let notice = match db::remove_person(&state.pool, id).await {
        Ok(()) => "Verwijderd.",
        Err(_) => "De lootjes zijn al getrokken: maak de loting eerst ongedaan.",
    };
    admin_page(&state, Some(notice)).await
}

async fn draw(State(state): State<AppState>, jar: CookieJar) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    let notice = match db::make_draw(&state.pool).await {
        Ok(n) => format!("De lootjes zijn getrokken voor {n} mensen."),
        Err(e) => format!("Niet getrokken: {e}"),
    };
    admin_page(&state, Some(&notice)).await
}

/// Mails every participant a fresh link once the lots are drawn. One that
/// fails does not stop the rest; the notice names it.
async fn announce(State(state): State<AppState>, jar: CookieJar) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    if !db::drawn(&state.pool).await? {
        return admin_page(&state, Some("Trek eerst de lootjes.")).await;
    }
    let (mut sent, mut not_sent) = (0, Vec::new());
    for person in db::participants(&state.pool).await? {
        let valid = Duration::days(INVITE_LINK_DAYS);
        match send_link(&state, &person, &mail::Kind::Drawn, valid).await {
            Ok(true) => sent += 1,
            Ok(false) => not_sent.push(person.name),
            Err(e) => {
                tracing::warn!("announcing the draw to {}: {:#}", person.email, e.0);
                not_sent.push(person.name);
            }
        }
    }
    let notice = if not_sent.is_empty() {
        format!("Bericht verstuurd naar {sent} mensen.")
    } else {
        format!(
            "Bericht verstuurd naar {sent} mensen; niet naar {} (probeer het over een minuut opnieuw).",
            not_sent.join(", ")
        )
    };
    admin_page(&state, Some(&notice)).await
}

#[derive(Deserialize)]
struct ConfirmForm {
    confirm: Option<String>,
}

async fn reset_draw(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<ConfirmForm>,
) -> Page {
    if the_sint(&state, &jar).await?.is_none() {
        return Ok(Redirect::to("/").into_response());
    }
    if form.confirm.as_deref() != Some("yes") {
        return admin_page(
            &state,
            Some("Vink het vakje aan om de loting ongedaan te maken."),
        )
        .await;
    }
    db::reset_draw(&state.pool).await?;
    admin_page(&state, Some("De loting is ongedaan gemaakt.")).await
}
