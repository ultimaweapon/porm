use crate::parse;

#[test]
fn parse_with_valid() {
    // Parse.
    let mut out = Vec::new();
    let migrations = [
        "CREATE TABLE foo (key serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (key));",
        "CREATE TABLE bar (bar smallint);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
        "ALTER TABLE foo ADD disabled boolean NOT NULL DEFAULT FALSE;",
    ];

    parse(&mut out, migrations).unwrap();

    // Check output.
    let out = String::from_utf8(out).unwrap();

    assert_eq!(
        out,
        r#"use porm::migration::Migration;
use std::borrow::Cow;
use std::fmt::Write;
use std::time::SystemTime;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Error, GenericClient, Row};

pub struct Foo<'a> {
    pub key: i32,
    pub value: Option<i64>,
    pub desc: Option<Cow<'a, str>>,
    pub disabled: bool,
}

impl<'a> Foo<'a> {
    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO foo (key, value, desc, disabled) VALUES ($1, $2, $3, $4)", &[&self.key, &self.value, &self.desc, &self.disabled]).await?;
        Ok(())
    }

    pub async fn find<T: GenericClient>(client: &T, key: i32) -> Result<Option<Self>, Error> {
        let r = client.query_opt("SELECT * FROM foo WHERE key = $1", &[&key]).await?;
        let r = match r {
            Some(v) => v,
            None => return Ok(None),
        };

        Self::from_row(r).map(Some)
    }

    fn from_row(r: Row) -> Result<Self, Error> {
        let key = r.try_get::<_, i32>("key")?;
        let value = r.try_get::<_, Option<i64>>("value")?;
        let desc = r.try_get::<_, Option<String>>("desc")?;
        let disabled = r.try_get::<_, bool>("disabled")?;

        Ok(Self { key, value, desc: desc.map(Cow::Owned), disabled })
    }
}

pub struct FooBuilder<'a> {
    key: Option<i32>,
    value: Option<Option<i64>>,
    desc: Option<Option<&'a str>>,
    disabled: Option<bool>,
}

impl<'a> FooBuilder<'a> {
    pub fn new() -> Self {
        Self { key: None, value: None, desc: None, disabled: None }
    }

    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<Foo<'static>, Error> {
        let mut sql = String::with_capacity(1024);
        let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity(4);

        sql.push_str("INSERT INTO foo (key, value, desc, disabled) VALUES (");

        if let Some(v) = &self.key {
            values.push(v);
            write!(sql, "${}", values.len()).unwrap();
        } else {
            sql.push_str("DEFAULT");
        }

        if let Some(v) = &self.value {
            values.push(v);
            write!(sql, ", ${}", values.len()).unwrap();
        } else {
            sql.push_str(", DEFAULT");
        }

        if let Some(v) = &self.desc {
            values.push(v);
            write!(sql, ", ${}", values.len()).unwrap();
        } else {
            sql.push_str(", DEFAULT");
        }

        if let Some(v) = &self.disabled {
            values.push(v);
            write!(sql, ", ${}", values.len()).unwrap();
        } else {
            sql.push_str(", DEFAULT");
        }

        sql.push_str(") RETURNING *");

        client.query_one(&sql, &values).await.and_then(Foo::from_row)
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

    fn from_row(r: Row) -> Result<Self, Error> {
        let bar = r.try_get::<_, Option<i16>>("bar")?;

        Ok(Self { bar })
    }
}

pub struct BarBuilder {
    bar: Option<Option<i16>>,
}

impl BarBuilder {
    pub fn new() -> Self {
        Self { bar: None }
    }

    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<Bar, Error> {
        let mut sql = String::with_capacity(1024);
        let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity(1);

        sql.push_str("INSERT INTO bar (bar) VALUES (");

        if let Some(v) = &self.bar {
            values.push(v);
            write!(sql, "${}", values.len()).unwrap();
        } else {
            sql.push_str("DEFAULT");
        }

        sql.push_str(") RETURNING *");

        client.query_one(&sql, &values).await.and_then(Bar::from_row)
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

    fn from_row(r: Row) -> Result<Self, Error> {
        let baz = r.try_get::<_, Option<SystemTime>>("baz")?;

        Ok(Self { baz })
    }
}

pub struct FooBarBuilder {
    baz: Option<Option<SystemTime>>,
}

impl FooBarBuilder {
    pub fn new() -> Self {
        Self { baz: None }
    }

    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<FooBar, Error> {
        let mut sql = String::with_capacity(1024);
        let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity(1);

        sql.push_str("INSERT INTO foo_bar (baz) VALUES (");

        if let Some(v) = &self.baz {
            values.push(v);
            write!(sql, "${}", values.len()).unwrap();
        } else {
            sql.push_str("DEFAULT");
        }

        sql.push_str(") RETURNING *");

        client.query_one(&sql, &values).await.and_then(FooBar::from_row)
    }
}

pub static MIGRATIONS: [Migration; 3] = [
    Migration {
        name: None,
        script: "CREATE TABLE foo (key serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (key));",
    },
    Migration {
        name: None,
        script: "CREATE TABLE bar (bar smallint);CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    },
    Migration {
        name: None,
        script: "ALTER TABLE foo ADD disabled boolean NOT NULL DEFAULT FALSE;",
    },
];
"#
    );
}
