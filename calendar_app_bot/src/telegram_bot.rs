use anyhow::Error;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures::{try_join, StreamExt};
use lazy_static::lazy_static;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use telegram_bot::{
    Api, CanReplySendMessage, CanSendMessage, ChatId, ChatRef, MessageKind, ToChatRef, UpdateKind,
    UserId,
};
use tokio::{
    sync::RwLock,
    time::{self, sleep, timeout},
};

use calendar_app_lib::{
    calendar_sync::CalendarSync, config::Config, models::AuthorizedUsers, pgpool::PgPool,
};

use crate::failure_count::FailureCount;

type UserIds = RwLock<HashMap<UserId, Option<ChatId>>>;

lazy_static! {
    static ref TELEGRAM_USERIDS: UserIds = RwLock::new(HashMap::new());
    static ref FAILURE_COUNT: FailureCount = FailureCount::new(5);
}

#[derive(Clone)]
pub struct TelegramBot {
    api: Arc<Api>,
    pool: PgPool,
    cal_sync: Arc<CalendarSync>,
}

impl TelegramBot {
    pub async fn new(bot_token: &str, pool: &PgPool, config: &Config) -> Self {
        Self {
            api: Arc::new(Api::new(bot_token)),
            pool: pool.clone(),
            cal_sync: Arc::new(CalendarSync::new(config.clone(), pool.clone()).await),
        }
    }

    pub async fn run(&self) -> Result<(), Error> {
        let fill_task = self.fill_telegram_user_ids();
        let notification_task = self.notification_handler();
        let bot_task = self.telegram_worker();
        try_join!(fill_task, notification_task, bot_task).map(|_| ())
    }

    pub async fn telegram_worker(&self) -> Result<(), Error> {
        loop {
            FAILURE_COUNT.check()?;
            match timeout(time::Duration::from_secs(3600), self.bot_handler()).await {
                Ok(Ok(_)) | Err(_) => FAILURE_COUNT.reset()?,
                Ok(Err(_)) => FAILURE_COUNT.increment()?,
            }
        }
    }

    pub async fn bot_handler(&self) -> Result<(), Error> {
        let mut stream = self.api.stream();
        while let Some(update) = stream.next().await {
            FAILURE_COUNT.check()?;
            if let UpdateKind::Message(message) = update?.kind {
                FAILURE_COUNT.check()?;
                if let MessageKind::Text { ref data, .. } = message.kind {
                    FAILURE_COUNT.check()?;
                    if TELEGRAM_USERIDS.read().await.contains_key(&message.from.id) {
                        FAILURE_COUNT.check()?;
                        if let ChatRef::Id(chat_id) = message.chat.to_chat_ref() {
                            if data.starts_with("/init") {
                                self.update_telegram_chat_id(message.from.id, chat_id)
                                    .await?;
                                self.api
                                    .send(
                                        message.text_reply(format!(
                                            "Initializing chat_id {}",
                                            chat_id
                                        )),
                                    )
                                    .await?;
                            } else if data.starts_with("/cal") {
                                for event in self.cal_sync.list_agenda(0, 1).await? {
                                    self.send_message(
                                        chat_id,
                                        &event
                                            .get_summary(
                                                &self.cal_sync.config.domain,
                                                &self.pool,
                                                self.cal_sync.config.default_time_zone,
                                            )
                                            .await,
                                    )
                                    .await?;
                                }
                            }
                        }
                    } else {
                        // Answer message with "Hi".
                        self.api
                            .send(message.text_reply(format!(
                                "Hi, {}, user_id {}! You just wrote '{}'",
                                &message.from.first_name, &message.from.id, data
                            )))
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn notification_handler(&self) -> Result<(), Error> {
        let now = Utc::now();
        let mut events: VecDeque<_> = self.cal_sync.list_agenda(0, 1).await?.collect();
        let mut agenda_datetime = DateTime::<Utc>::from_utc(
            NaiveDateTime::new(
                NaiveDate::from_ymd(now.year(), now.month(), now.day()),
                NaiveTime::from_hms(12, 0, 0),
            ),
            Utc,
        );
        loop {
            FAILURE_COUNT.check()?;
            let now = Utc::now();
            for chat_id in TELEGRAM_USERIDS.read().await.values() {
                if let Some(chat_id) = chat_id {
                    if now > agenda_datetime {
                        agenda_datetime = agenda_datetime + Duration::days(1);
                        events = self.cal_sync.list_agenda(0, 1).await?.collect();
                        for event in &events {
                            self.send_message(
                                *chat_id,
                                &event
                                    .get_summary(
                                        &self.cal_sync.config.domain,
                                        &self.pool,
                                        self.cal_sync.config.default_time_zone,
                                    )
                                    .await,
                            )
                            .await?;
                        }
                    } else {
                        while let Some(event) = events.front() {
                            if now > event.start_time - Duration::minutes(5) {
                                self.send_message(
                                    *chat_id,
                                    &event
                                        .get_summary(
                                            &self.cal_sync.config.domain,
                                            &self.pool,
                                            self.cal_sync.config.default_time_zone,
                                        )
                                        .await,
                                )
                                .await?;
                                events.pop_front();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            sleep(time::Duration::from_secs(60)).await;
        }
    }

    pub async fn send_message(&self, chat: ChatId, msg: &str) -> Result<(), Error> {
        self.api.spawn(chat.text(msg));
        Ok(())
    }

    async fn fill_telegram_user_ids(&self) -> Result<(), Error> {
        loop {
            FAILURE_COUNT.check()?;
            let p = self.pool.clone();
            if let Ok(authorized_users) = AuthorizedUsers::get_authorized_users(&p).await {
                let telegram_userid_map: HashMap<_, _> = authorized_users
                    .into_iter()
                    .filter_map(|user| {
                        user.telegram_userid.map(|userid| {
                            (UserId::new(userid), user.telegram_chatid.map(ChatId::new))
                        })
                    })
                    .collect();
                *TELEGRAM_USERIDS.write().await = telegram_userid_map;
                FAILURE_COUNT.reset()?;
            } else {
                FAILURE_COUNT.increment()?;
            }
            sleep(time::Duration::from_secs(60)).await;
        }
    }

    async fn update_telegram_chat_id(&self, userid: UserId, chatid: ChatId) -> Result<(), Error> {
        if let Ok(authorized_users) = AuthorizedUsers::get_authorized_users(&self.pool).await {
            if let Some(mut user) = authorized_users
                .into_iter()
                .find(|user| user.telegram_userid == Some(userid.into()))
            {
                user.telegram_chatid.replace(chatid.into());
                user.update_authorized_users(&self.pool).await?;
                if let Some(telegram_chatid) = TELEGRAM_USERIDS.write().await.get_mut(&userid) {
                    telegram_chatid.replace(chatid);
                }
            }
            FAILURE_COUNT.reset()?;
        } else {
            FAILURE_COUNT.increment()?;
        }
        Ok(())
    }
}
