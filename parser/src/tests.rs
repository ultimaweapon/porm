use crate::migration::StrProvider;
use crate::parse;

#[test]
fn parse_with_valid() {
    // Parse.
    let mut out = Vec::new();
    let migrations = StrProvider::new([
        "CREATE TABLE foo (key serial NOT NULL, value bigint, PRIMARY KEY (key));",
        "CREATE TABLE bar (bar text);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    ]);

    parse(&mut out, migrations).unwrap();

    // Check output.
    let out = String::from_utf8(out).unwrap();

    assert_eq!(
        out,
        r#"use porm::migration::Migration;
use tokio_postgres::{Error, GenericClient};

pub struct Foo {
    pub key: i32,
    pub value: Option<i64>,
}

impl Foo {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO foo (key, value) VALUES ($1, $2)", &[&self.key, &self.value]).await?;
        Ok(())
    }
}

pub struct Bar {
    pub bar: Option<String>,
}

impl Bar {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO bar (bar) VALUES ($1)", &[&self.bar]).await?;
        Ok(())
    }
}

pub struct FooBar {
    pub baz: Option<::std::time::SystemTime>,
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
        script: "CREATE TABLE foo (key serial NOT NULL, value bigint, PRIMARY KEY (key));",
    },
    Migration {
        name: None,
        script: "CREATE TABLE bar (bar text);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    },
];
"#
    );
}
