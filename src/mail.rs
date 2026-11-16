//! The two emails the site sends: an invitation, and a sign-in link. Both carry
//! a one-use link; neither carries anything about the draw.

use anyhow::{Context, Result};
use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::Mail;

pub enum Kind {
    Invite,
    SignIn,
}

pub fn compose(kind: &Kind, name: &str, link: &str) -> (String, String) {
    match kind {
        Kind::Invite => (
            String::from("Uitnodiging: Sinterklaas Surprise"),
            format!(
                "Beste {name},\n\n\
                 De Sint nodigt je uit voor de Sinterklaas Surprise! Via deze link\n\
                 kom je binnen, vul je je verlanglijstje in en zie je later voor wie\n\
                 jij een surprise maakt:\n\n{link}\n\n\
                 De link werkt één keer en blijft zeven dagen geldig. Daarna vraag je\n\
                 op de website gewoon een nieuwe aan met je e-mailadres.\n\n\
                 Groeten van de Sint\n"
            ),
        ),
        Kind::SignIn => (
            String::from("Je inloglink voor Sinterklaas Surprise"),
            format!(
                "Beste {name},\n\n\
                 Met deze link log je in bij de Sinterklaas Surprise:\n\n{link}\n\n\
                 De link werkt één keer en blijft een half uur geldig. Heb je hem\n\
                 niet zelf aangevraagd, dan kun je deze e-mail negeren.\n\n\
                 Groeten van de Sint\n"
            ),
        ),
    }
}

pub async fn send(
    mail: &Mail,
    to_name: &str,
    to_email: &str,
    kind: &Kind,
    link: &str,
) -> Result<()> {
    let (subject, body) = compose(kind, to_name, link);
    match mail {
        Mail::Log => {
            tracing::info!("MAIL_TO_LOG: to {to_email}: {subject}: {link}");
            Ok(())
        }
        Mail::Smtp {
            host,
            port,
            user,
            password,
            from,
        } => {
            let to = Mailbox::new(
                Some(to_name.to_string()),
                to_email.parse().context("recipient address")?,
            );
            let message = Message::builder()
                .from(from.parse().context("MAIL_FROM")?)
                .to(to)
                .subject(subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body)?;
            let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                .port(*port)
                .credentials(Credentials::new(user.clone(), password.clone()))
                .build();
            transport.send(message).await.context("sending the email")?;
            Ok(())
        }
    }
}
