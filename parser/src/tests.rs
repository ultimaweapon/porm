use crate::parse;

#[test]
fn parse_with_valid() {
    // Parse.
    let mut out = Vec::new();
    let migrations = [
        "CREATE TABLE foo (key serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (key));",
        "CREATE TABLE bar (bar smallint);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    ];

    parse(&mut out, migrations).unwrap();

    // Check output.
    let out = String::from_utf8(out).unwrap();

    assert_eq!(
        out,
        r#"use porm::migration::Migration;
use std::borrow::Cow;
use std::time::SystemTime;
use tokio_postgres::{Error, GenericClient};

pub struct Foo<'a> {
    pub key: i32,
    pub value: Option<i64>,
    pub desc: Option<Cow<'a, str>>,
}

impl<'a> Foo<'a> {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO foo (key, value, desc) VALUES ($1, $2, $3)", &[&self.key, &self.value, &self.desc]).await?;
        Ok(())
    }

    pub async fn find<T: GenericClient>(client: &T, key: i32) -> Result<Option<Self>, Error> {
        let r = client.query_opt("SELECT * FROM foo WHERE key = $1", &[&key]).await?;
        let r = match r {
            Some(v) => v,
            None => return Ok(None),
        };

        let key = r.try_get::<_, i32>("key")?;
        let value = r.try_get::<_, Option<i64>>("value")?;
        let desc = r.try_get::<_, Option<String>>("desc")?;

        Ok(Some(Self { key, value, desc: desc.map(Cow::Owned) }))
    }
}

pub struct Bar {
    pub bar: Option<i16>,
}

impl Bar {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO bar (bar) VALUES ($1)", &[&self.bar]).await?;
        Ok(())
    }
}

pub struct FooBar {
    pub baz: Option<SystemTime>,
}

impl FooBar {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO foo_bar (baz) VALUES ($1)", &[&self.baz]).await?;
        Ok(())
    }
}

pub static MIGRATIONS: [Migration; 2] = [
    Migration {
        name: None,
        script: "CREATE TABLE foo (key serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (key));",
    },
    Migration {
        name: None,
        script: "CREATE TABLE bar (bar smallint);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    },
];
"#
    );
}
