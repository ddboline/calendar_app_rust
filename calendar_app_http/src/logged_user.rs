pub use authorized_users::{
    get_random_key, get_secrets, token::Token, AuthorizedUser, AUTHORIZED_USERS, JWT_SECRET,
    KEY_LENGTH, SECRET_KEY, TRIGGER_DB_UPDATE, LOGIN_HTML,
};
use futures::TryStreamExt;
use log::debug;
use maplit::hashset;
use rweb::{filters::cookie::cookie, Filter, Rejection, Schema};
use rweb_helper::UuidWrapper;
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use std::{
    convert::{TryFrom, TryInto},
    env,
    str::FromStr,
};
use uuid::Uuid;

use calendar_app_lib::{models::AuthorizedUsers as AuthorizedUsersDB, pgpool::PgPool};

use crate::errors::ServiceError as Error;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Schema)]
#[schema(component="LoggedUser")]
pub struct LoggedUser {
    #[schema(description = "Email Address")]
    pub email: StackString,
    #[schema(description = "Session Id")]
    pub session: UuidWrapper,
    #[schema(description = "Secret Key")]
    pub secret_key: StackString,
}

impl LoggedUser {
    /// # Errors
    /// Return error if `session_id` doesn't match `self.session`
    pub fn verify_session_id(&self, session_id: Uuid) -> Result<(), Error> {
        if self.session == session_id {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    #[must_use]
    pub fn filter() -> impl Filter<Extract = (Self,), Error = Rejection> + Copy {
        cookie("session-id")
            .and(cookie("jwt"))
            .and_then(|id: Uuid, user: Self| async move {
                user.verify_session_id(id)
                    .map(|_| user)
                    .map_err(rweb::reject::custom)
            })
    }
}

impl From<AuthorizedUser> for LoggedUser {
    fn from(user: AuthorizedUser) -> Self {
        Self {
            email: user.email,
            session: user.session.into(),
            secret_key: user.secret_key,
        }
    }
}

impl TryFrom<Token> for LoggedUser {
    type Error = Error;
    fn try_from(token: Token) -> Result<Self, Self::Error> {
        if let Ok(user) = token.try_into() {
            if AUTHORIZED_USERS.is_authorized(&user) {
                return Ok(user.into());
            }
            debug!("NOT AUTHORIZED {:?}", user);
        }
        Err(Error::Unauthorized)
    }
}

impl FromStr for LoggedUser {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut buf = StackString::new();
        buf.push_str(s);
        let token: Token = buf.into();
        token.try_into()
    }
}

/// # Errors
/// Return error if `get_authorized_users` fails
pub async fn fill_from_db(pool: &PgPool) -> Result<(), Error> {
    debug!("{:?}", *TRIGGER_DB_UPDATE);
    let users = if TRIGGER_DB_UPDATE.check() {
        AuthorizedUsersDB::get_authorized_users(pool)
            .await?
            .map_ok(|user| user.email)
            .try_collect()
            .await?
    } else {
        AUTHORIZED_USERS.get_users()
    };
    if let Ok("true") = env::var("TESTENV").as_ref().map(String::as_str) {
        AUTHORIZED_USERS.update_users(hashset! {"user@test".into()});
    }
    AUTHORIZED_USERS.update_users(users);
    debug!("{:?}", *AUTHORIZED_USERS);
    Ok(())
}
