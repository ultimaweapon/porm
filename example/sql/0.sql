CREATE TABLE posts (
    id serial NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    published boolean NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id)
);

CREATE INDEX ON posts (published);
