use self::db::{Post, PostBuilder};
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

    // Test builder.
    let p = PostBuilder::new("Foo", "Bar.").create(&pg).await.unwrap();

    assert_eq!(p.id, 1);
    assert_eq!(p.title, "Foo");
    assert_eq!(p.body, "Bar.");
    assert!(!p.published);

    // Test find.
    let p1 = Post::find(&pg, 1).await.unwrap().unwrap();
    let p2 = Post::find(&pg, 2).await.unwrap();

    assert_eq!(p1.title, "Foo");
    assert!(p2.is_none());

    // Shutdown.
    drop(pg);

    con.await.unwrap();
}
