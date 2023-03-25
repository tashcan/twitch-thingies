use mysql_async::prelude::*;

use tracing::info;

mod settings;
use settings::*;

mod embedded {
    use rust_embed::*;

    #[derive(RustEmbed)]
    #[folder = "migrations"]
    pub(super) struct Asset;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::new()?;

    let builder = mysql_async::OptsBuilder::from_opts(
        mysql_async::Opts::from_url(&settings.database_url).unwrap(),
    );
    let pool = mysql_async::Pool::new(builder.ssl_opts(mysql_async::SslOpts::default()));
    let mut conn = pool.get_conn().await?;

    if let Err(ref e @ mysql_async::Error::Server(ref s @ mysql_async::ServerError { code, .. })) =
        r"CREATE TABLE `schema_history` (
    `version` int NOT NULL,
    `name` varchar(255),
    `applied_on` varchar(255),
    `checksum` varchar(255),
    PRIMARY KEY (`version`));
    "
        .ignore(&mut conn)
        .await
    {
        if code != 1050 {
            anyhow::bail!("Shits fucked");
        }
    }

    #[derive(Ord, PartialOrd, Eq)]
    struct MigrationFile {
        version: u32,
        file: String,
    }

    impl PartialEq for MigrationFile {
        fn eq(&self, other: &Self) -> bool {
            self.version < other.version
        }
    }

    let mut sorted_migration_files = vec![];
    for file in embedded::Asset::iter() {
        let version = file.split("_").next().unwrap().parse::<u32>().unwrap();
        let file = file.to_owned();
        sorted_migration_files.push(MigrationFile {
            version,
            file: file.to_string(),
        });
    }
    sorted_migration_files.sort();
    let latest_version: u32 = *"SELECT version FROM schema_history ORDER BY version DESC LIMIT 1"
        .with(())
        .map(&mut conn, |version| version)
        .await?
        .get(0)
        .unwrap_or(&0);

    use std::time::SystemTime;

    for migration in sorted_migration_files {
        if latest_version < migration.version {
            if let Some(file) = embedded::Asset::get(&migration.file) {
                info!("Applying {}", migration.version);
                conn.query_iter(file.data).await?;
                r"INSERT INTO schema_history (version, name, applied_on, checksum)
      VALUES (:version, :name, :applied_on, :checksum)"
                    .with([
                          params! {
                              "version" => migration.version,
                              "name" => migration.file.clone(),
                              "applied_on" => {
                                  SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
                              },
                              "checksum" => 0
                          }
                    ])
                    .batch(&mut conn)
                    .await?;
                println!("{} {}", migration.file, migration.version);
            }
        }
    }

    Ok(())
}
