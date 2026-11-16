# sinterklaas

The family's Sinterklaas surprise at `sinterklaas.xinutec.org`. The Sint invites
people by email; each signs in with a one-use link, keeps a wish list, and after
the draw sees whom they make a surprise for and that person's list. Nobody,
the Sint included, can see anyone else's lot.

Rust (axum) rendering plain HTML forms, MariaDB, no frontend build. Pushed as
`xinutec/sinterklaas:latest` by CI; the manifests are kubes'
`dhall/apps/sinterklaas.dhall`.

## How it works

- **Invite only.** The Sint (`ADMIN_EMAIL`, made admin at every start) adds
  people on the Grote Boek page (`/admin`) and sends each an invitation. The
  Sint only keeps the book: no wish list, not in the draw. Whoever runs it takes
  part under their own address like everyone else. Asking
  for a link with an address that is not invited answers the same as one that
  is, so the form does not say who takes part.
- **Links.** A sign-in link is valid for 30 minutes, an invitation for 7 days,
  each once. Only their SHA-256 is stored. Opening a link shows a button;
  signing in takes the press, so a mail scanner following the link does not
  spend it. One link a minute per person.
- **Sessions** are a random token in an HttpOnly cookie, 60 days, stored hashed.
- **Every form post** must carry this site's `Origin` (or `Referer`).
- **The draw** is a uniformly random derangement over the participants:
  nobody draws themselves, everyone is drawn once. It is made once; the Sint
  can undo it, and nobody can be added or removed while it stands.

## Configuration

| Variable | |
|---|---|
| `DATABASE_URL` | `mysql://user:pw@host/sinterklaas` |
| `PUBLIC_URL` | `https://sinterklaas.xinutec.org`, used in the links and the origin check |
| `ADMIN_EMAIL`, `ADMIN_NAME` | the Sint |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASSWORD`, `MAIL_FROM` | submission over implicit TLS (465) |
| `MAIL_TO_LOG=1` | development only: links go to the log instead of SMTP |
| `BIND_ADDR`, `STATIC_DIR` | default `0.0.0.0:8080`, `static` |

## Development

```sh
scripts/setup-hooks.sh        # the pre-commit gate (gate.json)
nix develop -c cargo test     # the flow test skips without a database
```

The gate runs the tests against a throwaway MariaDB on port 3320.
