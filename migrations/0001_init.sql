-- The people invited this year, the links that sign them in, their sessions,
-- and the draw.

CREATE TABLE person (
    id          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    name        VARCHAR(80)  NOT NULL,
    email       VARCHAR(254) NOT NULL,
    wishlist    TEXT         NOT NULL,
    is_admin    BOOLEAN      NOT NULL DEFAULT FALSE,
    invited_at  DATETIME(3)  NULL,
    created_at  DATETIME(3)  NOT NULL,
    updated_at  DATETIME(3)  NOT NULL,
    UNIQUE KEY person_email (email)
);

-- A sign-in link: only the SHA-256 of its token is stored, so a database read
-- cannot sign anyone in. Used once.
CREATE TABLE login_link (
    token_hash  CHAR(64)        NOT NULL PRIMARY KEY,
    person_id   BIGINT UNSIGNED NOT NULL,
    created_at  DATETIME(3)     NOT NULL,
    expires_at  DATETIME(3)     NOT NULL,
    used_at     DATETIME(3)     NULL,
    CONSTRAINT login_link_person FOREIGN KEY (person_id) REFERENCES person (id) ON DELETE CASCADE
);

CREATE TABLE session (
    token_hash  CHAR(64)        NOT NULL PRIMARY KEY,
    person_id   BIGINT UNSIGNED NOT NULL,
    expires_at  DATETIME(3)     NOT NULL,
    CONSTRAINT session_person FOREIGN KEY (person_id) REFERENCES person (id) ON DELETE CASCADE
);

-- Who buys for whom. One row per giver; nobody receives twice.
CREATE TABLE draw (
    giver_id    BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    receiver_id BIGINT UNSIGNED NOT NULL,
    drawn_at    DATETIME(3)     NOT NULL,
    UNIQUE KEY draw_receiver (receiver_id),
    CONSTRAINT draw_giver FOREIGN KEY (giver_id) REFERENCES person (id) ON DELETE CASCADE,
    CONSTRAINT draw_receiver_person FOREIGN KEY (receiver_id) REFERENCES person (id) ON DELETE CASCADE
);
