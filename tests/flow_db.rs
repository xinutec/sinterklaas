//! The whole site against a real MariaDB, through its router: a link signs in
//! once, a wish list saves, only the Sint reaches the Grote Boek, and the draw
//! gives everyone one other person. Runs when SINTERKLAAS_TEST_DATABASE_URL is
//! set (the gate brings a server up); skips otherwise.
//!
//! One test, because every step shares the one database and the draw is over
//! everyone in it.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::Duration;
use http_body_util::BodyExt;
use sinterklaas::config::{Config, Mail};
use sinterklaas::{db, routes, state::AppState, token};
use tower::ServiceExt;

const SITE: &str = "http://sinterklaas.test";

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Option<String>, String) {
    let res = app.clone().oneshot(req).await.expect("a response");
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::to_string);
    let body = res.into_body().collect().await.expect("a body").to_bytes();
    (status, cookie, String::from_utf8_lossy(&body).into_owned())
}

fn post(path: &str, cookie: Option<&str>, form: &str) -> Request<Body> {
    let mut b = Request::post(path)
        .header(header::ORIGIN, SITE)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    b.body(Body::from(form.to_string())).expect("a request")
}

fn get(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::get(path);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    b.body(Body::empty()).expect("a request")
}

/// Signs `email` in through a link made the way the site makes one.
async fn sign_in(app: &Router, pool: &sqlx::MySqlPool, email: &str) -> String {
    let person = db::person_by_email(pool, email)
        .await
        .expect("read")
        .expect("invited");
    let link = token::new_token();
    db::create_link(pool, person.id, &token::hash(&link), Duration::minutes(30))
        .await
        .expect("a link");
    let (status, cookie, _) = send(app, post(&format!("/in/{link}"), None, "")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    cookie.expect("a session cookie")
}

#[tokio::test]
async fn invite_sign_in_wish_and_draw() {
    let Ok(url) = std::env::var("SINTERKLAAS_TEST_DATABASE_URL") else {
        eprintln!("SINTERKLAAS_TEST_DATABASE_URL unset — skipping the flow test");
        return;
    };
    let pool = db::connect(&url).await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    for clear in [
        "DELETE FROM draw",
        "DELETE FROM session",
        "DELETE FROM login_link",
        "DELETE FROM person",
    ] {
        sqlx::query(clear)
            .execute(&pool)
            .await
            .expect("a clean slate");
    }
    db::ensure_admin(&pool, " Sint@Example.org ", "Sint")
        .await
        .expect("the Sint");
    let cfg = Config {
        database_url: url,
        bind_addr: String::new(),
        public_url: SITE.into(),
        static_dir: "static".into(),
        admin_email: "sint@example.org".into(),
        admin_name: "Sint".into(),
        mail: Mail::Log,
    };
    let app = routes::router(AppState::new(pool.clone(), cfg));

    // A post from elsewhere is refused before anything is read.
    let foreign = Request::post("/login")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("email=sint%40example.org"))
        .expect("a request");
    assert_eq!(send(&app, foreign).await.0, StatusCode::FORBIDDEN);

    // A stranger and an invited person get the same answer.
    let (_, _, stranger) = send(&app, post("/login", None, "email=nobody%40example.org")).await;
    let (_, _, invited) = send(&app, post("/login", None, "email=SINT%40example.org")).await;
    assert_eq!(stranger, invited);

    // The Sint adds people; only the Sint can.
    let sint = sign_in(&app, &pool, "sint@example.org").await;
    for (name, email) in [
        ("Anna", "anna@example.org"),
        ("Bram", "bram@example.org"),
        ("Cor", "cor@example.org"),
    ] {
        let (status, _, page) = send(
            &app,
            post(
                "/admin/people",
                Some(&sint),
                &format!("name={name}&email={email}"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(page.contains(name), "{page}");
    }
    let anna = sign_in(&app, &pool, "anna@example.org").await;
    let (_, _, page) = send(&app, get("/admin", Some(&anna))).await;
    assert!(
        !page.contains("Grote Boek van de Sint"),
        "Anna reached the admin page"
    );
    let (_, _, page) = send(&app, post("/admin/draw", Some(&anna), "")).await;
    assert!(!page.contains("getrokken"), "Anna drew");
    assert!(!db::drawn(&pool).await.expect("read"));

    // A link signs in once.
    let person = db::person_by_email(&pool, "bram@example.org")
        .await
        .expect("read")
        .expect("Bram");
    let link = token::new_token();
    db::create_link(&pool, person.id, &token::hash(&link), Duration::minutes(30))
        .await
        .expect("a link");
    let (_, _, page) = send(&app, get(&format!("/in/{link}"), None)).await;
    assert!(
        page.contains("Inloggen"),
        "opening the link must not sign in by itself"
    );
    assert_eq!(
        send(&app, post(&format!("/in/{link}"), None, "")).await.0,
        StatusCode::SEE_OTHER
    );
    let (_, cookie, page) = send(&app, post(&format!("/in/{link}"), None, "")).await;
    assert!(
        cookie.is_none() && page.contains("al gebruikt"),
        "a spent link signed in again"
    );

    // An expired link does not sign in.
    let old = token::new_token();
    db::create_link(&pool, person.id, &token::hash(&old), Duration::minutes(-1))
        .await
        .expect("a link");
    let (_, cookie, _) = send(&app, post(&format!("/in/{old}"), None, "")).await;
    assert!(cookie.is_none(), "an expired link signed in");

    // A wish list saves, and markup in it stays text.
    let (status, _, _) = send(
        &app,
        post(
            "/wishes",
            Some(&anna),
            "name=Anna&wishlist=-+sokken%0D%0A-+%3Cb%3Eboek%3C%2Fb%3E",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let saved = db::person_by_email(&pool, "anna@example.org")
        .await
        .expect("read")
        .expect("Anna");
    assert_eq!(saved.wishlist, "- sokken\n- <b>boek</b>");

    // The draw: four people, nobody themselves, everyone once; then refused again.
    let (_, _, page) = send(&app, post("/admin/draw", Some(&sint), "")).await;
    assert!(page.contains("getrokken voor 4"), "{page}");
    let people = db::people(&pool).await.expect("people");
    let mut received = Vec::new();
    for p in &people {
        let r = db::receiver_of(&pool, p.id)
            .await
            .expect("read")
            .expect("everyone draws");
        assert_ne!(r.id, p.id, "{} drew themselves", p.name);
        received.push(r.id);
    }
    received.sort_unstable();
    received.dedup();
    assert_eq!(received.len(), people.len(), "someone was drawn twice");
    assert!(
        db::make_draw(&pool).await.is_err(),
        "a second draw was made"
    );

    // Whoever drew Anna sees her list, escaped; Anna's own page never names
    // anyone but her own receiver.
    let mut giver = None;
    for p in &people {
        let r = db::receiver_of(&pool, p.id)
            .await
            .expect("read")
            .expect("drawn");
        if r.email == "anna@example.org" {
            giver = Some(p.clone());
        }
    }
    let giver = giver.expect("someone drew Anna");
    let giver_cookie = sign_in(&app, &pool, &giver.email).await;
    let (_, _, page) = send(&app, get("/", Some(&giver_cookie))).await;
    assert!(
        page.contains("Anna") && page.contains("&lt;b&gt;boek&lt;/b&gt;"),
        "{page}"
    );

    // Nobody can be removed once drawn; the Sint can undo the draw.
    assert!(db::remove_person(&pool, people[0].id).await.is_err());
    let (_, _, page) = send(&app, post("/admin/reset-draw", Some(&sint), "confirm=yes")).await;
    assert!(page.contains("ongedaan gemaakt"), "{page}");
    assert!(!db::drawn(&pool).await.expect("read"));
}
