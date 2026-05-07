ALTER TABLE post ADD published boolean NOT NULL DEFAULT FALSE;

CREATE INDEX ON post (published);
