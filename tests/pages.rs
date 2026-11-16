//! What a person's text can do to a page (nothing), and which posts are taken.

use axum::http::{HeaderMap, HeaderValue, header};
use sinterklaas::html::{esc, wishes};
use sinterklaas::routes::from_here;

#[test]
fn markup_in_a_wish_is_shown_not_run() {
    assert_eq!(
        esc(r#"<script>"x"&'y'</script>"#),
        "&lt;script&gt;&quot;x&quot;&amp;&#39;y&#39;&lt;/script&gt;"
    );
    let page = wishes("- <img src=x onerror=alert(1)>\nEen boek");
    assert!(!page.contains("<img"), "{page}");
    assert!(page.contains("&lt;img"), "{page}");
}

#[test]
fn dashes_and_stars_make_a_list_and_other_lines_paragraphs() {
    assert_eq!(
        wishes("Graag:\n- sokken\n* een boek\n\nVerder niets"),
        "<p>Graag:</p><ul><li>sokken</li><li>een boek</li></ul><p>Verder niets</p>"
    );
    assert_eq!(wishes("  \n"), "");
}

fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        h.insert(k.clone(), HeaderValue::from_str(v).expect("header"));
    }
    h
}

#[test]
fn only_a_post_from_the_site_itself_is_taken() {
    let site = "https://sinterklaas.xinutec.org";
    assert!(from_here(&headers(&[(header::ORIGIN, site)]), site));
    assert!(from_here(
        &headers(&[(header::REFERER, "https://sinterklaas.xinutec.org/admin")]),
        site
    ));
    assert!(!from_here(
        &headers(&[(header::ORIGIN, "https://evil.example")]),
        site
    ));
    assert!(
        !from_here(
            &headers(&[(
                header::REFERER,
                "https://sinterklaas.xinutec.org.evil.example/"
            )]),
            site
        ),
        "a host that merely starts with ours"
    );
    assert!(!from_here(&HeaderMap::new(), site), "no origin at all");
}
