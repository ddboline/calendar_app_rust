use anyhow::Error;
use diesel::{pg::PgConnection, r2d2::ConnectionManager};
use r2d2::{Pool, PooledConnection};
use std::fmt;

use crate::stack_string::StackString;

#[derive(Clone)]
pub struct PgPool {
    pgurl: StackString,
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

    pub fn get(&self) -> Result<PooledConnection<ConnectionManager<PgConnection>>, Error> {
        self.pool.get().map_err(Into::into)
    }
}
