# Porm

Porm is a new type of ORM for PostgreSQL. Instead of defining a model in Rust, Porm parse migration scripts and generate models from it.

## Features

- Lightweight.
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
