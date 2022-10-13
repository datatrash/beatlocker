use deadpool::async_trait;
use deadpool::managed::{Manager, RecycleResult};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection};

pub struct DbPool {
    connect_options: SqliteConnectOptions,
}

impl DbPool {
    pub fn new(connect_options: SqliteConnectOptions) -> Self {
        Self { connect_options }
    }
}

#[async_trait]
impl Manager for DbPool {
    type Type = SqliteConnection;
    type Error = sqlx::Error;

    async fn create(&self) -> Result<SqliteConnection, sqlx::Error> {
        SqliteConnection::connect_with(&self.connect_options).await
    }

    async fn recycle(&self, obj: &mut SqliteConnection) -> RecycleResult<sqlx::Error> {
        Ok(obj.ping().await?)
    }
}
