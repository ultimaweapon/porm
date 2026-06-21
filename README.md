# Porm
[![Crates.io Version](https://img.shields.io/crates/v/porm?label=porm)](https://crates.io/crates/porm)
[![Crates.io Version](https://img.shields.io/crates/v/porm-parser?label=porm-parser)](https://crates.io/crates/porm-parser)
[![docs.rs](https://img.shields.io/docsrs/porm)](https://docs.rs/porm)

Porm is a new type of ORM for PostgreSQL. Instead of defining some models in Rust, Porm parse migration scripts and generate models from it.

> [!WARNING]
> Porm currently in a pre-1.0 so prepare for a lot of breaking changes!

## Features

- Lightweight.
  - The generated models in a thin layer on top of [tokio-postgres](https://crates.io/crates/tokio-postgres) API.
- Database is a single source of truth.
- Built-in schema migration.
- Use actual parser from PostgreSQL.

## How it works

Suppose you have the following migration scripts:

```sql
-- 0.sql
CREATE TABLE post (
    id serial NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    PRIMARY KEY (id)
);
```

```sql
-- 1.sql
ALTER TABLE post ADD published boolean NOT NULL DEFAULT FALSE;

CREATE INDEX ON post (published);
```

Porm will generate a corresponding struct for you:

```rust
pub struct Post<'a> {
    pub id: i32,
    pub title: Cow<'a, str>,
    pub body: Cow<'a, str>,
    pub published: bool,
}
```

The generated struct will have methods to query PostgreSQL based on table indexes:

```rust
impl<'a> Post<'a> {
    pub async fn find<T: GenericClient>(client: &T, id: i32) -> Result<Option<Self>, Error>;
    pub async fn select_by_published<T: GenericClient>(
        client: &T,
        published: bool,
    ) -> Result<Pin<Box<impl Stream<Item = Result<Self, Error>> + use<'a, T>>>, Error>;
}
```

Porm also generate a helper struct to insert a new row with default values:

```rust
pub struct PostBuilder<'a> { ... }

impl<'a> PostBuilder<'a> {
    pub fn new(title: &'a str, body: &'a str) -> Self;
    pub fn set_id(&mut self, v: i32) -> &mut Self;
    pub fn set_title(&mut self, v: &'a str) -> &mut Self;
    pub fn set_body(&mut self, v: &'a str) -> &mut Self;
    pub fn set_published(&mut self, v: bool) -> &mut Self;
    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<Post<'static>, Error>;
}
```

Notice `PostBuilder::new` function that requires non-null columns without default value. The following code will insert a new row and construct `Post` value with `id` and `published` loaded from the database:

```rust
let post = PostBuilder::new("Foo", "Bar.").create(&pg).await.unwrap();

assert_eq!(post.id, 1); // Suppose this is the first row.
assert_eq!(post.title, "Foo");
assert_eq!(post.body, "Bar.");
assert!(!post.published);
```

## Non-goals

- Synchronous API.
- Supports downgrade migration.
- Supports other databases.

## Breaking changes in parser 0.3

- `ParseError` no longer have migration version on every variants.
- Migration version will be become a migration name if `Migration::name` return `None`.

## Breaking changes in 0.2

- `Migration::name` no longer optional.
- `Logger::run` has new signature.
- `Error::ExecuteMigration` and `Error::UpdateVersion` has been updated.

## License

This project is licensed under either of

- Apache License, Version 2.0
- MIT License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Porm by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
