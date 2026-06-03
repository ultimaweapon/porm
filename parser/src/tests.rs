use crate::parse;
use porm_config::{Config, SimplePluralizer};
use pretty_assertions::assert_str_eq;

#[test]
fn parse_with_valid() {
    // Set up config.
    let config = Config {
        pluralizer: &SimplePluralizer,
    };

    // Parse.
    let mut out = Vec::new();
    let migrations = [
        "CREATE TABLE account (id serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (id));",
        "CREATE TABLE blog (id serial NOT NULL, owner integer NOT NULL, FOREIGN KEY (owner) REFERENCES account (id));",
        "CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
        "ALTER TABLE account ADD disabled boolean NOT NULL DEFAULT FALSE;",
        "CREATE INDEX ON account USING hash (value);",
    ];

    parse(&mut out, &config, migrations).unwrap();

    // Check output.
    let out = String::from_utf8(out).unwrap();

    assert_str_eq!(
        out,
        r#"use futures::{Stream, TryStreamExt};
use porm::migration::Migration;
use std::borrow::Cow;
use std::fmt::Write;
use std::pin::{Pin, pin};
use std::time::SystemTime;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Error, GenericClient, Row};

pub struct Account<'a> {
    pub id: i32,
    pub value: Option<i64>,
    pub desc: Option<Cow<'a, str>>,
    pub disabled: bool,
    pub blogs: Vec<Blog>,
}

impl<'a> Account<'a> {
    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO account (id, value, desc, disabled) VALUES ($1, $2, $3, $4)", &[&self.id, &self.value, &self.desc, &self.disabled]).await?;
        Ok(())
    }

    pub async fn find<T: GenericClient>(client: &T, id: i32) -> Result<Option<Self>, Error> {
        let r = client.query_opt("SELECT * FROM account WHERE id = $1", &[&id]).await?;
        let r = match r {
            Some(v) => v,
            None => return Ok(None),
        };

        Self::from_row(r).map(Some)
    }

    pub async fn select_by_value<T: GenericClient>(client: &T, value: Option<i64>) -> Result<Pin<Box<impl Stream<Item = Result<Self, Error>> + use<'a, T>>>, Error> {
        let f = client.query_raw("SELECT * FROM account WHERE value = $1", [&value]).await?.and_then(|r| Self::from_row(r).map_or_else(futures::future::err, futures::future::ok));

        Ok(Box::pin(f))
    }

    fn from_row(r: Row) -> Result<Self, Error> {
        let id = r.try_get::<_, i32>("id")?;
        let value = r.try_get::<_, Option<i64>>("value")?;
        let desc = r.try_get::<_, Option<String>>("desc")?;
        let disabled = r.try_get::<_, bool>("disabled")?;

        Ok(Self { id, value, desc: desc.map(Cow::Owned), disabled })
    }
}

pub struct AccountQuery {
}

impl AccountQuery {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct AccountBuilder<'a> {
    id: Option<i32>,
    value: Option<Option<i64>>,
    desc: Option<Option<&'a str>>,
    disabled: Option<bool>,
}

impl<'a> AccountBuilder<'a> {
    pub fn new() -> Self {
        Self { id: None, value: None, desc: None, disabled: None }
    }

    pub fn set_id(&mut self, v: i32) -> &mut Self {
        self.id = Some(v);
        self
    }

    pub fn set_value(&mut self, v: Option<i64>) -> &mut Self {
        self.value = Some(v);
        self
    }

    pub fn set_desc(&mut self, v: Option<&'a str>) -> &mut Self {
        self.desc = Some(v);
        self
    }

    pub fn set_disabled(&mut self, v: bool) -> &mut Self {
        self.disabled = Some(v);
        self
    }

    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<Account<'static>, Error> {
        let mut sql = String::with_capacity(1024);
        let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity(4);

        sql.push_str("INSERT INTO account (id, value, desc, disabled) VALUES (");

        if let Some(v) = &self.id {
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

        client.query_one(&sql, &values).await.and_then(Account::from_row)
    }
}

pub struct Blog {
    pub id: i32,
    pub owner: i32,
}

impl Blog {
    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO blog (id, owner) VALUES ($1, $2)", &[&self.id, &self.owner]).await?;
        Ok(())
    }

    fn from_row(r: Row) -> Result<Self, Error> {
        let id = r.try_get::<_, i32>("id")?;
        let owner = r.try_get::<_, i32>("owner")?;

        Ok(Self { id, owner })
    }
}

pub struct BlogQuery {
}

impl BlogQuery {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct BlogBuilder {
    id: Option<i32>,
    owner: i32,
}

impl BlogBuilder {
    pub fn new(owner: i32) -> Self {
        Self { id: None, owner }
    }

    pub fn set_id(&mut self, v: i32) -> &mut Self {
        self.id = Some(v);
        self
    }

    pub fn set_owner(&mut self, v: i32) -> &mut Self {
        self.owner = v;
        self
    }

    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<Blog, Error> {
        let mut sql = String::with_capacity(1024);
        let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity(2);

        sql.push_str("INSERT INTO blog (id, owner) VALUES (");

        if let Some(v) = &self.id {
            values.push(v);
            write!(sql, "${}", values.len()).unwrap();
        } else {
            sql.push_str("DEFAULT");
        }

        values.push(&self.owner);
        write!(sql, ", ${}", values.len()).unwrap();

        sql.push_str(") RETURNING *");

        client.query_one(&sql, &values).await.and_then(Blog::from_row)
    }
}

pub struct FooBar {
    pub baz: Option<SystemTime>,
}

impl FooBar {
    pub async fn create<T: GenericClient>(&self, client: &T) -> Result<(), Error> {
        client.execute("INSERT INTO foo_bar (baz) VALUES ($1)", &[&self.baz]).await?;
        Ok(())
    }

    fn from_row(r: Row) -> Result<Self, Error> {
        let baz = r.try_get::<_, Option<SystemTime>>("baz")?;

        Ok(Self { baz })
    }
}

pub struct FooBarQuery {
}

impl FooBarQuery {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct FooBarBuilder {
    baz: Option<Option<SystemTime>>,
}

impl FooBarBuilder {
    pub fn new() -> Self {
        Self { baz: None }
    }

    pub fn set_baz(&mut self, v: Option<SystemTime>) -> &mut Self {
        self.baz = Some(v);
        self
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

pub static MIGRATIONS: [Migration; 5] = [
    Migration {
        name: "0",
        script: "CREATE TABLE account (id serial NOT NULL, value bigint, \"desc\" text, PRIMARY KEY (id));",
    },
    Migration {
        name: "1",
        script: "CREATE TABLE blog (id serial NOT NULL, owner integer NOT NULL, FOREIGN KEY (owner) REFERENCES account (id));",
    },
    Migration {
        name: "2",
        script: "CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    },
    Migration {
        name: "3",
        script: "ALTER TABLE account ADD disabled boolean NOT NULL DEFAULT FALSE;",
    },
    Migration {
        name: "4",
        script: "CREATE INDEX ON account USING hash (value);",
    },
];
"#
    );
}
