//! The pages, rendered on the server. Every value from a person passes through
//! `esc`; nothing else reaches the HTML.

use crate::db::Person;

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="nl">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link href="https://fonts.googleapis.com/css2?family=Mountains+of+Christmas:wght@700&family=Lora:ital@0;1&display=swap" rel="stylesheet">
<link href="/static/style.css" rel="stylesheet">
</head>
<body>
<div class="falling-container" id="rain"></div>
<div id="countdown">Nog <span id="timer">...</span> tot Pakjesavond!</div>
<div class="container">
<h1>Sinterklaas Surprise</h1>
{body}
</div>
<script src="/static/festive.js"></script>
</body>
</html>
"#,
        title = esc(title),
    )
}

/// A wish list as its author wrote it: lines starting `-` or `*` are list
/// items, any other line a paragraph.
pub fn wishes(text: &str) -> String {
    let mut out = String::new();
    let mut in_list = false;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(item) = line.strip_prefix('-').or_else(|| line.strip_prefix('*')) {
            if !in_list {
                out.push_str("<ul>");
                in_list = true;
            }
            out.push_str(&format!("<li>{}</li>", esc(item.trim())));
        } else {
            if in_list {
                out.push_str("</ul>");
                in_list = false;
            }
            out.push_str(&format!("<p>{}</p>", esc(line)));
        }
    }
    if in_list {
        out.push_str("</ul>");
    }
    out
}

/// The sign-in form; `email` is put back in the field after a refusal.
pub fn sign_in(notice: Option<&str>, email: &str) -> String {
    let notice = notice
        .map(|n| format!(r#"<p class="notice error">{}</p>"#, esc(n)))
        .unwrap_or_default();
    page(
        "Sinterklaas Surprise",
        &format!(
            r#"<p>Welkom, Helper van de Sint!<br>Vul je e-mailadres in, dan stuurt de Sint je een inloglink.</p>
{notice}
<form method="post" action="/login">
<input type="email" name="email" value="{email}" placeholder="jij@voorbeeld.nl" autocomplete="email" required>
<br>
<button type="submit">Stuur mij een link</button>
</form>
<p class="hint center">Alleen wie door de Sint is uitgenodigd kan inloggen.</p>"#,
            email = esc(email)
        ),
    )
}

pub fn check_mail() -> String {
    page(
        "Kijk in je mail",
        r#"<p class="notice">Er ligt nu een inloglink in je mailbox. Hij werkt één keer en blijft een half uur geldig.</p>
<p><a href="/">Terug</a></p>"#,
    )
}

/// The page a link opens. Signing in takes a button press, not the visit: a
/// mail scanner that follows the link must not spend it.
pub fn confirm_link(token: &str) -> String {
    page(
        "Inloggen",
        &format!(
            r#"<p>Welkom terug bij de Sint!</p>
<form method="post" action="/in/{token}">
<button type="submit">Inloggen</button>
</form>"#,
            token = esc(token)
        ),
    )
}

pub fn link_spent() -> String {
    page(
        "Link verlopen",
        r#"<p class="notice error">Deze link is al gebruikt of verlopen.</p>
<p><a href="/">Vraag een nieuwe link aan</a></p>"#,
    )
}

pub fn home(me: &Person, receiver: Option<&Person>, saved: bool) -> String {
    let saved = if saved {
        r#"<p class="notice">Opgeslagen! De Sint heeft je lijstje.</p>"#
    } else {
        ""
    };
    let draw = match receiver {
        None => String::from(
            r#"<p class="empty">De Sint heeft nog niet geloot. Kom later terug om te zien voor wie jij een surprise maakt.</p>"#,
        ),
        Some(r) => {
            let list = if r.wishlist.trim().is_empty() {
                String::from(r#"<p class="empty">Nog niets ingevuld. Kijk later nog eens!</p>"#)
            } else {
                wishes(&r.wishlist)
            };
            format!(
                r#"<h2>Jij maakt een surprise voor:<br><span class="target-name">{name}</span></h2>
<div class="wishlist-container">{list}</div>"#,
                name = esc(&r.name)
            )
        }
    };
    page(
        "Sinterklaas Surprise",
        &format!(
            r#"<p>Hallo {name}!</p>
{saved}
<section>
<h2 class="section">Jouw lootje</h2>
{draw}
</section>
<section>
<h2 class="section">Jouw verlanglijstje</h2>
<form method="post" action="/wishes">
<label for="name">Je naam</label>
<input id="name" name="name" value="{name}" maxlength="80" required>
<label for="wishlist">Wat wens je je?</label>
<textarea id="wishlist" name="wishlist" maxlength="4000">{wishlist}</textarea>
<button type="submit">Opslaan</button>
</form>
</section>
<div class="actions">
<form method="post" action="/logout"><button type="submit" class="quiet">Uitloggen</button></form>
</div>"#,
            name = esc(&me.name),
            wishlist = esc(&me.wishlist),
        ),
    )
}

pub fn admin(people: &[Person], drawn: bool, notice: Option<&str>) -> String {
    let notice = notice
        .map(|n| format!(r#"<p class="notice">{}</p>"#, esc(n)))
        .unwrap_or_default();
    let rows: String = people
        .iter()
        .map(|p| {
            let invited = if p.invited_at.is_some() {
                format!(
                    r#"uitgenodigd <form method="post" action="/admin/invite/{id}"><button type="submit" class="quiet">opnieuw</button></form>"#,
                    id = p.id
                )
            } else {
                format!(
                    r#"<form method="post" action="/admin/invite/{id}"><button type="submit">uitnodigen</button></form>"#,
                    id = p.id
                )
            };
            let remove = if drawn {
                String::new()
            } else {
                format!(
                    r#"<form method="post" action="/admin/remove/{id}"><button type="submit" class="quiet">verwijderen</button></form>"#,
                    id = p.id
                )
            };
            let list = if p.wishlist.trim().is_empty() { "nog leeg" } else { "ingevuld" };
            format!(
                r#"<tr><td>{name}<br><span class="hint email">{email}</span></td><td>{list}</td><td>{invited} {remove}</td></tr>"#,
                name = esc(&p.name),
                email = esc(&p.email),
            )
        })
        .collect();
    let draw = if drawn {
        r#"<p>De lootjes zijn getrokken. Iedereen ziet alleen zijn eigen lootje; de Sint ziet er geen.</p>
<form method="post" action="/admin/announce"><button type="submit">Iedereen laten weten</button></form>
<p class="hint">Stuurt iedereen een e-mail met een inloglink. Wie wie getrokken heeft, staat er niet in.</p>
<form method="post" action="/admin/reset-draw">
<label><input type="checkbox" name="confirm" value="yes" required style="width:auto;margin:0 8px 0 0">Ja, ik wil de loting ongedaan maken</label>
<button type="submit" class="quiet">Loting ongedaan maken</button>
</form>"#
    } else {
        r#"<p>Als iedereen erbij is: trek de lootjes. Niemand trekt zichzelf.</p>
<form method="post" action="/admin/draw"><button type="submit">Lootjes trekken</button></form>"#
    };
    let add = if drawn {
        r#"<p>Maak eerst de loting ongedaan om iemand toe te voegen.</p>"#
    } else {
        r#"<form method="post" action="/admin/people">
<label for="name">Naam</label>
<input id="name" name="name" maxlength="80" required>
<label for="email">E-mailadres</label>
<input id="email" name="email" type="email" maxlength="254" required>
<br><br>
<button type="submit">Toevoegen</button>
</form>"#
    };
    page(
        "Het Grote Boek",
        &format!(
            r#"<p>Het Grote Boek van de Sint</p>
{notice}
<section>
<h2 class="section">Deelnemers</h2>
<table><tr><th>Wie</th><th>Lijstje</th><th></th></tr>{rows}</table>
</section>
<section>
<h2 class="section">Iemand toevoegen</h2>
{add}
</section>
<section>
<h2 class="section">De loting</h2>
{draw}
</section>
<div class="actions"><form method="post" action="/logout"><button type="submit" class="quiet">Uitloggen</button></form></div>"#
        ),
    )
}
