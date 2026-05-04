use tokio_postgres::NoTls;

mod db;

fn main() {
    let tokio = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build_local(Default::default())
        .unwrap();

    tokio.block_on(run())
}

async fn run() {
    // Conntect to PostgreSQL.
    let config = "host=localhost user=postgres password=postgres";
    let (pg, con) = tokio_postgres::connect(config, NoTls).await.unwrap();
    let con = tokio::task::spawn_local(async move { con.await.unwrap() });

    // Migrate database.
    let logger = std::io::stdout();
    let history_table = "migrations";

    porm::migration::migrate(&pg, logger, history_table, &self::db::MIGRATIONS)
        .await
        .unwrap();

    // Shutdown.
    drop(pg);

    con.await.unwrap();
}
