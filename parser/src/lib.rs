//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this crate will be used from build script.
pub use self::error::*;

use self::migration::Migration;
use self::model::{Field, Model};
use self::ty::Type;
use self::writer::Writer;
use heck::AsUpperCamelCase;
use memchr::Memchr;
use pg_query::protobuf::{AlterTableStmt, AlterTableType, ColumnDef, ConstrType, CreateStmt};
use pg_query::{Node, NodeEnum};
use proc_macro2::Literal;
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::NonZero;
use std::ops::Index;
use std::path::{MAIN_SEPARATOR, Path};

pub mod migration;

mod error;
mod model;
#[cfg(test)]
mod tests;
mod ty;
mod writer;

/// Parse SQL migration scripts in directory `migrations` and generate models from it.
///
/// `f` will be used to get migration identifier from its path. The order of parsing will be
/// determine by migration identifier produced by this function.
///
/// Files in nested directories or extension other than `sql` will be ignored.
///
/// This will set environment variable `PORM_GENERATED_FILE` to the generated file. It also emit
/// `cargo::rerun-if-changed` for `migrations`.
///
/// # Panics
/// If `f` returns a duplicated identifier.
pub fn parse_for_build_script<K>(
    migrations: impl AsRef<str>,
    mut f: impl FnMut(&Path) -> Result<K, Box<dyn std::error::Error>>,
) -> Result<(), ParseError>
where
    K: Ord,
{
    // List all SQL files.
    let migrations = migrations.as_ref();
    let mut files = BTreeMap::new();

    println!("cargo::rerun-if-changed={migrations}");

    for e in std::fs::read_dir(migrations).map_err(ParseError::ReadDirectory)? {
        // Skip if directory.
        let e = e.map_err(ParseError::ReadDirectory)?;
        let t = e.file_type().map_err(ParseError::ReadDirectory)?;

        if t.is_dir() {
            continue;
        }

        // Skip if not SQL file.
        let p = e.path();

        if p.extension().is_none_or(|v| !v.eq_ignore_ascii_case("sql")) {
            continue;
        }

        // Get key.
        let k = match f(&p) {
            Ok(v) => v,
            Err(e) => return Err(ParseError::GetMigrationId(p, e)),
        };

        assert!(files.insert(k, p).is_none());
    }

    // Build path to output file.
    let mut path = std::env::var("OUT_DIR").map_err(ParseError::GetOutputDir)?;

    path.push(MAIN_SEPARATOR);
    path.push_str("models.rs");

    // Create output file.
    let mut out = match File::create(&path) {
        Ok(v) => BufWriter::new(v),
        Err(e) => return Err(ParseError::WriteCode(e)),
    };

    // Parse.
    let files = files.into_values();

    parse(&mut out, files)?;

    out.flush().map_err(ParseError::WriteCode)?;

    // Set PORM_GENERATED_FILE.
    println!("cargo::rustc-env=PORM_GENERATED_FILE={path}");

    Ok(())
}

/// Parse SQL migration scripts and generate models from it.
///
/// The order of items produced by `migrations` must be the same every time.
///
/// Use [parse_for_build_script()] instead if you are calling from build.rs.
pub fn parse<M>(out: impl Write, migrations: impl IntoIterator<Item = M>) -> Result<(), ParseError>
where
    M: Migration,
{
    // Load migrations.
    let scripts = migrations.into_iter();
    let mut migrations = Vec::new();
    let mut models = FxHashMap::default();

    for (version, migration) in scripts.enumerate() {
        // Get migration.
        let name = migration.name();
        let script = match migration.read() {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ReadMigration(name, version, e)),
        };

        // Parse migration.
        let parsed = match pg_query::parse(&script) {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ParseMigration(name, version, e)),
        };

        for stmt in parsed.protobuf.stmts {
            // Get statement.
            let node = match stmt.stmt.and_then(|v| v.node) {
                Some(v) => v,
                None => continue,
            };

            // Set up parse context.
            let mut cx = Context {
                models: &mut models,
                script: &script,
            };

            // Process.
            let r = match node {
                NodeEnum::CreateStmt(n) => parse_create_stmt(&mut cx, &name, version, n),
                NodeEnum::AlterTableStmt(n) => parse_alter_table_stmt(&mut cx, &name, version, n),
                _ => continue,
            };

            if let Some(Err(e)) = r {
                return Err(e);
            }
        }

        migrations.push((name, script));
    }

    generate(migrations, models, out).map_err(ParseError::WriteCode)
}

fn parse_create_stmt(
    cx: &mut Context,
    mn: &Option<String>,
    mv: usize,
    node: CreateStmt,
) -> Option<Result<(), ParseError>> {
    use std::collections::hash_map::Entry;

    // Check table name.
    let table = node.relation.map(|v| v.relname)?;

    if table.chars().any(|c| c.is_uppercase()) {
        return Some(Err(ParseError::UnsupportedTableName(mn.clone(), mv, table)));
    }

    // Parse create statement.
    let defs = node.table_elts;
    let mut model = Model::default();

    for def in defs {
        let def = match def.node {
            Some(v) => v,
            None => continue,
        };

        match def {
            NodeEnum::ColumnDef(v) => {
                let loc = v.location;

                if let Err(e) = parse_column_def(&mut model, v) {
                    return Some(Err(ParseError::Column(mn.clone(), mv, cx.get_line(loc), e)));
                }
            }
            NodeEnum::Constraint(v) => {
                let loc = v.location;

                if let Err(e) = model.parse_table_constraint(v) {
                    return Some(Err(ParseError::TableConstraint(
                        mn.clone(),
                        mv,
                        cx.get_line(loc),
                        e,
                    )));
                }
            }
            _ => (),
        }
    }

    // Check if exists.
    let e = match cx.models.entry(table) {
        Entry::Occupied(e) => {
            return Some(Err(ParseError::DuplicatedTable(
                mn.clone(),
                mv,
                e.key().clone(),
            )));
        }
        Entry::Vacant(e) => e,
    };

    e.insert(model);

    Some(Ok(()))
}

fn parse_alter_table_stmt(
    cx: &mut Context,
    mn: &Option<String>,
    mv: usize,
    node: AlterTableStmt,
) -> Option<Result<(), ParseError>> {
    let table = node.relation?.relname;
    let model = match cx.models.get_mut(&table) {
        Some(v) => v,
        None => return Some(Err(ParseError::UnknownTable(mn.clone(), mv, table))),
    };

    for cmd in node.cmds {
        let cmd = match cmd.node {
            Some(NodeEnum::AlterTableCmd(n)) => n,
            _ => continue,
        };

        match cmd.subtype.try_into().unwrap() {
            AlterTableType::AtAddColumn => match cmd.def.and_then(|v| v.node) {
                Some(NodeEnum::ColumnDef(n)) => {
                    let loc = n.location;

                    if let Err(e) = parse_column_def(model, n) {
                        return Some(Err(ParseError::Column(mn.clone(), mv, cx.get_line(loc), e)));
                    }
                }
                _ => continue,
            },
            _ => continue,
        }
    }

    Some(Ok(()))
}

fn parse_column_def(model: &mut Model, node: Box<ColumnDef>) -> Result<(), ColumnError> {
    use indexmap::map::Entry;

    // Check column name.
    let name = node.colname;

    if name.chars().any(|c| c.is_uppercase()) {
        return Err(ColumnError::UnsupportedName(name));
    }

    // Parse definition.
    let ty = node.type_name.unwrap();
    let ty = parse_column_type(model, ty.names);
    let mut nullable = true;
    let mut has_default = false;

    for c in node.constraints {
        if let Some(NodeEnum::Constraint(v)) = c.node {
            match v.contype.try_into().unwrap() {
                ConstrType::ConstrNull => nullable = true,
                ConstrType::ConstrNotnull => nullable = false,
                ConstrType::ConstrDefault => has_default = true,
                _ => (),
            }
        }
    }

    // Check if exists.
    let e = match model.fields.entry(name) {
        Entry::Occupied(e) => return Err(ColumnError::Duplicated(e.key().clone())),
        Entry::Vacant(e) => e,
    };

    e.insert(Field {
        ty,
        nullable,
        has_default,
    });

    Ok(())
}

fn parse_column_type(model: &mut Model, nodes: Vec<Node>) -> Type {
    let mut nodes = nodes.into_iter();
    let name = match nodes.next().unwrap().node.unwrap() {
        NodeEnum::String(v) => v.sval,
        v => todo!("{v:?}"),
    };

    match name.as_str() {
        "pg_catalog" => nodes.next().map(parse_system_type).unwrap(),
        "serial" => Type::Serial,
        "text" => {
            model.has_lifetime = true;

            Type::Text
        }
        v => todo!("{v}"),
    }
}

fn parse_system_type(node: Node) -> Type {
    let name = match node.node.unwrap() {
        NodeEnum::String(v) => v.sval,
        v => todo!("{v:?}"),
    };

    match name.as_str() {
        "bool" => Type::Boolean,
        "int2" => Type::SmallInt,
        "int4" => Type::Integer,
        "int8" => Type::BigInt,
        "timestamptz" => Type::TimestampWithTz,
        v => todo!("{v}"),
    }
}

fn generate(
    migrations: Vec<(Option<String>, String)>,
    models: FxHashMap<String, Model>,
    out: impl Write,
) -> Result<(), std::io::Error> {
    // Generate preamble.
    let mut w = Writer::new(out);

    w.line("use porm::migration::Migration;")?;
    w.line("use std::borrow::Cow;")?;
    w.line("use std::fmt::Write;")?;
    w.line("use std::time::SystemTime;")?;
    w.line("use tokio_postgres::types::ToSql;")?;
    w.line("use tokio_postgres::{Error, GenericClient, Row};")?;

    // Write models.
    for (table, model) in models {
        let name = AsUpperCamelCase(&table);

        w.blank_line()?;
        w.begin(format_args!(r#"pub struct {}"#, name))?;

        if model.has_lifetime {
            w.end("<'a> {")?;
        } else {
            w.end(" {")?;
        }

        w.increase_indent();

        for (c, f) in &model.fields {
            w.begin(format_args!(r#"pub {c}: "#))?;

            if f.nullable {
                w.end(format_args!(r#"Option<{}>,"#, f.ty.for_field()))?;
            } else {
                w.end(format_args!(r#"{},"#, f.ty.for_field()))?;
            }
        }

        w.decrease_indent();
        w.line(r#"}"#)?;
        w.blank_line()?;

        if model.has_lifetime {
            w.line(format_args!("impl<'a> {name}<'a> {{"))?;
        } else {
            w.line(format_args!("impl {name} {{"))?;
        }

        w.increase_indent();

        // Write insert method.
        w.line("pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {")?;

        w.increase_indent();
        w.begin(format_args!(r#"client.execute("INSERT INTO {table} ("#))?;

        for (i, c) in model.fields.keys().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("{c}"))?;
        }

        w.append(") VALUES (")?;

        for (i, n) in (1..=model.fields.len()).enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("${n}"))?;
        }

        w.append(r#")", &["#)?;

        for (i, c) in model.fields.keys().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("&self.{c}"))?;
        }

        w.end(r#"]).await?;"#)?;

        w.line("Ok(())")?;
        w.decrease_indent();

        w.line("}")?;

        // Write find method.
        if !model.primary_key.is_empty() {
            w.blank_line()?;
            w.begin("pub async fn find<T: GenericClient>(client: &T")?;

            for c in &model.primary_key {
                let f = model.fields.get(c).unwrap();

                if f.nullable {
                    w.append(format_args!(", {}: Option<{}>", c, f.ty.for_param()))?;
                } else {
                    w.append(format_args!(", {}: {}", c, f.ty.for_param()))?;
                }
            }

            w.end(") -> Result<Option<Self>, Error> {")?;

            w.increase_indent();
            w.begin(format_args!(
                r#"let r = client.query_opt("SELECT * FROM {} WHERE "#,
                table
            ))?;

            for (i, c) in model.primary_key.iter().enumerate() {
                if i != 0 {
                    w.append("AND ")?;
                }

                w.append(format_args!("{} = ${}", c, i + 1))?;
            }

            w.append(r#"", &["#)?;

            for (i, c) in model.primary_key.iter().enumerate() {
                let f = model.fields.get(c).unwrap();

                if i != 0 {
                    w.append(", ")?;
                }

                if f.nullable || f.ty.pass_by_ref() {
                    w.append(format_args!("&{c}"))?;
                } else {
                    w.append(format_args!("{c}"))?;
                }
            }

            w.end("]).await?;")?;
            w.line("let r = match r {")?;

            w.increase_indent();
            w.line("Some(v) => v,")?;
            w.line("None => return Ok(None),")?;
            w.decrease_indent();

            w.line("};")?;
            w.blank_line()?;

            w.line("Self::from_row(r).map(Some)")?;

            w.decrease_indent();

            w.line("}")?;
        }

        // Write from_row method.
        w.blank_line()?;
        w.line("fn from_row(r: Row) -> Result<Self, Error> {")?;
        w.increase_indent();

        for (c, f) in &model.fields {
            if f.nullable {
                w.line(format_args!(
                    r#"let {} = r.try_get::<_, Option<{}>>("{}")?;"#,
                    c,
                    f.ty.for_retrieve(),
                    c
                ))?;
            } else {
                w.line(format_args!(
                    r#"let {} = r.try_get::<_, {}>("{}")?;"#,
                    c,
                    f.ty.for_retrieve(),
                    c
                ))?;
            }
        }

        w.blank_line()?;
        w.begin(r#"Ok(Self { "#)?;

        for (i, (n, f)) in model.fields.iter().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            if f.ty.is_cow() {
                if f.nullable {
                    w.append(format_args!("{n}: {n}.map(Cow::Owned)"))?;
                } else {
                    w.append(format_args!("{n}: Cow::Owned({n})"))?;
                }
            } else {
                w.append(format_args!("{n}"))?;
            }
        }

        w.end(" })")?;

        w.decrease_indent();
        w.line("}")?;

        w.decrease_indent();
        w.line("}")?;

        // Write builder struct.
        w.blank_line()?;

        if model.has_lifetime {
            w.line(format_args!("pub struct {name}Builder<'a> {{"))?;
        } else {
            w.line(format_args!("pub struct {name}Builder {{"))?;
        }

        w.increase_indent();

        for (c, f) in &model.fields {
            w.begin(format_args!(r#"{c}: "#))?;

            if f.nullable {
                w.end(format_args!(r#"Option<Option<{}>>,"#, f.ty.for_builder()))?;
            } else if matches!(f.ty, Type::Serial) || f.has_default {
                w.end(format_args!(r#"Option<{}>,"#, f.ty.for_builder()))?;
            } else {
                w.end(format_args!(r#"{},"#, f.ty.for_builder()))?;
            }
        }

        w.decrease_indent();
        w.line("}")?;

        w.blank_line()?;

        if model.has_lifetime {
            w.line(format_args!("impl<'a> {name}Builder<'a> {{"))?;
        } else {
            w.line(format_args!("impl {name}Builder {{"))?;
        }

        w.increase_indent();

        // Write new for builder.
        w.begin("pub fn new(")?;

        for (i, (c, f)) in model
            .fields
            .iter()
            .filter(|(_, f)| !f.is_optional())
            .enumerate()
        {
            if i != 0 {
                w.append(", ")?;
            }

            w.end(format_args!(r#"{}: {}"#, c, f.ty.for_builder()))?;
        }

        w.end(") -> Self {")?;
        w.increase_indent();
        w.begin("Self { ")?;

        for (i, (c, f)) in model.fields.iter().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            if f.is_optional() {
                w.append(format_args!(r#"{c}: None"#))?;
            } else {
                w.append(format_args!(r#"{c}"#))?;
            }
        }

        w.end(" }")?;
        w.decrease_indent();
        w.line("}")?;

        // Write create for builder.
        w.blank_line()?;

        generate_builder_create(&mut w, &name, &table, &model)?;

        w.decrease_indent();
        w.line("}")?;
    }

    // Write migrations.
    w.blank_line()?;

    w.line(format_args!(
        r#"pub static MIGRATIONS: [Migration; {}] = ["#,
        migrations.len()
    ))?;

    w.increase_indent();

    for (name, script) in migrations {
        match name {
            Some(name) => {
                w.line("Migration {")?;

                w.increase_indent();
                w.line(format_args!("name: Some({}),", Literal::string(&name)))?;
                w.line(format_args!("script: {},", Literal::string(&script)))?;
                w.decrease_indent();

                w.line("},")?;
            }
            None => {
                w.line("Migration {")?;

                w.increase_indent();
                w.line("name: None,")?;
                w.line(format_args!("script: {},", Literal::string(&script)))?;
                w.decrease_indent();

                w.line("},")?;
            }
        }
    }

    w.decrease_indent();
    w.line("];")?;

    Ok(())
}

fn generate_builder_create<T>(
    w: &mut Writer<T>,
    name: impl Display,
    table: &str,
    model: &Model,
) -> Result<(), std::io::Error>
where
    T: Write,
{
    if model.has_lifetime {
        w.line(format_args!(
            "pub async fn create<T: GenericClient>(&self, client: &T) -> Result<{}<'static>, Error> {{",
            name
        ))?;
    } else {
        w.line(format_args!(
            "pub async fn create<T: GenericClient>(&self, client: &T) -> Result<{}, Error> {{",
            name
        ))?;
    }

    w.increase_indent();
    w.line("let mut sql = String::with_capacity(1024);")?;
    w.line(format_args!(
        "let mut values = Vec::<&(dyn ToSql + Sync)>::with_capacity({});",
        model.fields.len()
    ))?;

    w.blank_line()?;
    w.begin(r#"sql.push_str("INSERT INTO "#)?;
    w.append(table)?;
    w.append(" (")?;

    for (i, c) in model.fields.keys().enumerate() {
        if i != 0 {
            w.append(", ")?;
        }

        w.append(c.as_str())?;
    }

    w.end(r#") VALUES (");"#)?;

    // Generate values.
    for (i, (c, f)) in model.fields.iter().enumerate() {
        w.blank_line()?;

        if f.is_optional() {
            w.line(format_args!("if let Some(v) = &self.{c} {{"))?;
            w.increase_indent();
            w.line("values.push(v);")?;

            if i != 0 {
                w.line(r#"write!(sql, ", ${}", values.len()).unwrap();"#)?;
            } else {
                w.line(r#"write!(sql, "${}", values.len()).unwrap();"#)?;
            }

            w.decrease_indent();
            w.line("} else {")?;
            w.increase_indent();

            if i != 0 {
                w.line(r#"sql.push_str(", DEFAULT");"#)?;
            } else {
                w.line(r#"sql.push_str("DEFAULT");"#)?;
            }

            w.decrease_indent();
            w.line("}")?;
        } else if i != 0 {
            w.line(format_args!("values.push(&self.{c});"))?;
            w.line(r#"write!(sql, ", ${}", values.len()).unwrap();"#)?;
        } else {
            w.line(format_args!("values.push(&self.{c});"))?;
            w.line(r#"write!(sql, "${}", values.len()).unwrap();"#)?;
        }
    }

    w.blank_line()?;
    w.line(r#"sql.push_str(") RETURNING *");"#)?;

    // Generate a call to query_one.
    w.blank_line()?;
    w.line(format_args!(
        "client.query_one(&sql, &values).await.and_then({name}::from_row)"
    ))?;

    w.decrease_indent();
    w.line("}")?;

    Ok(())
}

/// Context to parse migration scripts.
struct Context<'a> {
    models: &'a mut FxHashMap<String, Model>,
    script: &'a str,
}

impl<'a> Context<'a> {
    fn get_line(&self, loc: i32) -> NonZero<u32> {
        let loc = usize::try_from(loc).unwrap();
        let script = self.script.as_bytes().index(..loc);
        let ln = Memchr::new(b'\n', script).count() + 1;

        ln.try_into().ok().and_then(NonZero::new).unwrap()
    }
}
