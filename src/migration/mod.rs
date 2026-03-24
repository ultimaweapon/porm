//! Migration management.
pub use self::logger::*;

use std::fmt::{Display, Formatter};
use tokio_postgres::Client;
use tokio_postgres::error::SqlState;

mod logger;

/// Migrate database to latest version.
///
/// `history_table` is the name of table to store migrations history so it should be hard-coded.
/// This table will be created automatically if not exists.
///
/// `migrations` can be obtained from the code that was generated with
/// [porm-parser](https://crates.io/crates/porm-parser).
///
/// # Warning
/// All queries on `history_table` will be construct with [format] macro. That mean `history_table`
/// should not come from untrusted source.
pub async fn migrate(
    client: &Client,
    mut logger: impl Logger,
    history_table: &str,
    migrations: &[Migration],
) -> Result<(), Error> {
    // Get current version.
    let mut sql = format!("SELECT version FROM {history_table} ORDER BY version DESC LIMIT 1");
    let current: Option<i32> = match client.query_opt_scalar(&sql, &[]).await {
        Ok(v) => v,
        Err(e) if e.code() == Some(&SqlState::UNDEFINED_TABLE) => {
            logger.create_history_table(history_table);

            sql = format!(
                "CREATE TABLE {history_table} (version integer NOT NULL, name text, applied_time timestamp with time zone NOT NULL, PRIMARY KEY (version))"
            );

            client
                .batch_execute(&sql)
                .await
                .map_err(Error::CreateHistoryTable)?;

            None
        }
        Err(e) => return Err(Error::QueryVersion(e)),
    };

    // Apply migrations.
    let current = current
        .map(usize::try_from)
        .transpose()
        .map_err(|_| Error::InvalidVersion)?;
    let next = current.map(|v| v + 1).unwrap_or(0);
    let sql =
        format!("INSERT INTO {history_table} (version, name, applied_time) VALUES ($1, $2, now())");

    logger.start(current);

    for next in next.. {
        let m = match migrations.get(next) {
            Some(v) => v,
            None => break,
        };

        logger.run(m.name, next);

        client
            .batch_execute(m.script)
            .await
            .map_err(|e| Error::ExecuteMigration(m.name, next, e))?;

        // Update version.
        let version = i32::try_from(next).unwrap();

        client
            .execute(&sql, &[&version, &m.name])
            .await
            .map_err(|e| Error::UpdateVersion(m.name, next, e))?;
    }

    Ok(())
}

/// Contains information for a migration.
pub struct Migration {
    /// Name of migration.
    pub name: Option<&'static str>,
    /// SQL statements for the migration.
    pub script: &'static str,
}

/// Reason when failed to apply SQL migrations.
#[derive(Debug)]
pub enum Error {
    /// Couldn't create table for migrations history.
    CreateHistoryTable(tokio_postgres::Error),
    /// Couldn't query database version.
    QueryVersion(tokio_postgres::Error),
    /// Current database version is invalid.
    InvalidVersion,
    /// Couldn't execute migration.
    ExecuteMigration(Option<&'static str>, usize, tokio_postgres::Error),
    /// Couldn't update database version.
    UpdateVersion(Option<&'static str>, usize, tokio_postgres::Error),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateHistoryTable(e) => Some(e),
            Self::QueryVersion(e) => Some(e),
            Self::InvalidVersion => None,
            Self::ExecuteMigration(_, _, e) => Some(e),
            Self::UpdateVersion(_, _, e) => Some(e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateHistoryTable(_) => {
                write!(f, "couldn't create table for migrations history")
            }
            Self::QueryVersion(_) => f.write_str("couldn't query database version"),
            Self::InvalidVersion => f.write_str("current database version is invalid"),
            Self::ExecuteMigration(n, v, _) => match n {
                Some(n) => write!(f, "couldn't execute migration {n}"),
                None => write!(f, "couldn't execute migration for version {v}"),
            },
            Self::UpdateVersion(_, v, _) => write!(f, "couldn't update database version to {v}"),
        }
    }
}
