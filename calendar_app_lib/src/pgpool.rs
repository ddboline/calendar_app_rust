use anyhow::Error;
use derive_more::Deref;
use diesel::{pg::PgConnection, r2d2::ConnectionManager};
use r2d2::{Pool, PooledConnection};
use std::fmt;

use stack_string::StackString;

pub type PgPoolConn = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(Clone, Deref)]
pub struct PgPool {
    pgurl: StackString,
    #[deref]
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl fmt::Debug for PgPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PgPool {}", self.pgurl)
    }
}

impl PgPool {
    pub fn new(pgurl: &str) -> Self {
        let manager = ConnectionManager::new(pgurl);
        Self {
            pgurl: pgurl.into(),
            pool: Pool::builder()
                .min_idle(Some(2))
                .build(manager)
                .expect("Failed to open DB connection"),
        }
    }

    pub fn get(&self) -> Result<PgPoolConn, Error> {
        self.pool.get().map_err(Into::into)
    }
}
