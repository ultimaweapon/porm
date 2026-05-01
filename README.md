# Porm
[![Crates.io Version](https://img.shields.io/crates/v/porm?label=porm)](https://crates.io/crates/porm)
[![Crates.io Version](https://img.shields.io/crates/v/porm-macros?label=porm-macros)](https://crates.io/crates/porm-macros)
[![Crates.io Version](https://img.shields.io/crates/v/porm-parser?label=porm-parser)](https://crates.io/crates/porm-parser)

Porm is a new type of ORM for PostgreSQL. Instead of defining some models in Rust, Porm parse migration scripts and generate models from it.

> [!WARNING]
> Porm currently in a pre-1.0 so prepare for a lot of breaking changes!

## Features

- Lightweight.
  - The generated models in a thin layer on top of [tokio-postgres](https://crates.io/crates/tokio-postgres) API.
- Database is a single source of truth.
- Built-in schema migration.
- Use actual parser from PostgreSQL.

## Non-goals

- Synchronous API.
- Supports downgrade migration.
- Supports other databases.

## License

This project is licensed under either of

- Apache License, Version 2.0
- MIT License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Porm by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
