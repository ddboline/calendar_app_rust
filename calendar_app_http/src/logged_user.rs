pub use authorized_users::{
    get_random_key, get_secrets, token::Token, AuthorizedUser, AUTHORIZED_USERS, JWT_SECRET,
    KEY_LENGTH, SECRET_KEY, TRIGGER_DB_UPDATE,
};
use log::debug;
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use std::{
    convert::{TryFrom, TryInto},
    env,
    str::FromStr,
};

use calendar_app_lib::{models::AuthorizedUsers as AuthorizedUsersDB, pgpool::PgPool};

use crate::errors::ServiceError as Error;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct LoggedUser {
    pub email: StackString,
}

impl From<AuthorizedUser> for LoggedUser {
    fn from(user: AuthorizedUser) -> Self {
        Self { email: user.email }
    }
}

impl From<LoggedUser> for AuthorizedUser {
    fn from(user: LoggedUser) -> Self {
        Self { email: user.email }
    }
}

impl TryFrom<Token> for LoggedUser {
    type Error = Error;
    fn try_from(token: Token) -> Result<Self, Self::Error> {
        let user = token.try_into()?;
        if AUTHORIZED_USERS.is_authorized(&user) {
            Ok(user.into())
        } else {
            debug!("NOT AUTHORIZED {:?}", user);
            Err(Error::Unauthorized)
        }
    }
}

impl FromStr for LoggedUser {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let token: Token = s.to_string().into();
        token.try_into()
    }
}

pub async fn fill_from_db(pool: &PgPool) -> Result<(), Error> {
    debug!("{:?}", *TRIGGER_DB_UPDATE);
    let users: Vec<_> = if TRIGGER_DB_UPDATE.check() {
        AuthorizedUsersDB::get_authorized_users(&pool)
            .await?
            .into_iter()
            .map(|user| AuthorizedUser { email: user.email })
            .collect()
    } else {
        AUTHORIZED_USERS.get_users()
    };
    if let Ok("true") = env::var("TESTENV").as_ref().map(String::as_str) {
        let user = AuthorizedUser {
            email: "user@test".into(),
        };
        AUTHORIZED_USERS.merge_users(&[user])?;
    }
    AUTHORIZED_USERS.merge_users(&users)?;
    debug!("{:?}", *AUTHORIZED_USERS);
    Ok(())
}
