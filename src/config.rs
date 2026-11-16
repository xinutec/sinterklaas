//! Runtime configuration, read from the environment at startup.

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug)]
pub struct Config {
    /// `mysql://sinterklaas:pw@host/sinterklaas`.
    pub database_url: String,
    pub bind_addr: String,
    /// Where the site is reached, without a trailing slash: the links in the
    /// emails point here, and a form post must come from here.
    pub public_url: String,
    pub static_dir: String,
    /// The Sint: made an admin at startup, so the first invite needs no
    /// database edit.
    pub admin_email: String,
    pub admin_name: String,
    pub mail: Mail,
}

/// How a sign-in link reaches its person.
#[derive(Clone, Debug)]
pub enum Mail {
    /// SMTP submission with STARTTLS, as the fleet's Mailu takes it.
    Smtp {
        host: String,
        port: u16,
        user: String,
        password: String,
        from: String,
    },
    /// Development only: the link is written to the log. Chosen by setting
    /// `MAIL_TO_LOG=1`, never by leaving SMTP unset, so a deploy missing its
    /// mail settings fails at startup instead of logging people's links.
    Log,
}

fn var(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("{name} is not set"))
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let mail = if std::env::var("MAIL_TO_LOG").as_deref() == Ok("1") {
            Mail::Log
        } else {
            Mail::Smtp {
                host: var("SMTP_HOST")?,
                port: var("SMTP_PORT")?.parse().context("SMTP_PORT")?,
                user: var("SMTP_USER")?,
                password: var("SMTP_PASSWORD")?,
                from: var("MAIL_FROM")?,
            }
        };
        let public_url = var("PUBLIC_URL")?.trim_end_matches('/').to_string();
        if !public_url.starts_with("http") {
            bail!("PUBLIC_URL must be an http(s) URL, got {public_url:?}");
        }
        Ok(Config {
            database_url: var("DATABASE_URL")?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            public_url,
            static_dir: std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into()),
            admin_email: var("ADMIN_EMAIL")?.trim().to_lowercase(),
            admin_name: var("ADMIN_NAME")?,
            mail,
        })
    }
}
