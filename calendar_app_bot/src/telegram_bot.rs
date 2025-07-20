use anyhow::Error;
use arc_swap::ArcSwap;
use deadqueue::unlimited::Queue;
use futures::{StreamExt, TryStreamExt, future, try_join};
use im::HashMap;
use stack_string::{StackString, format_sstr};
use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock},
};
use telegram_bot::{
    Api, CanReplySendMessage, CanSendMessage, ChatId, ChatRef, MessageKind, ToChatRef, UpdateKind,
    UserId, types::Update,
};
use time::{Duration, OffsetDateTime};
use tokio::{
    select,
    time::{sleep, timeout},
};

use calendar_app_lib::{
    calendar_sync::CalendarSync, config::Config, models::AuthorizedUsers, pgpool::PgPool,
};

use crate::failure_count::FailureCount;

type UserIds = ArcSwap<HashMap<UserId, Option<ChatId>>>;

static TELEGRAM_USERIDS: LazyLock<UserIds> =
    LazyLock::new(|| ArcSwap::new(Arc::new(HashMap::new())));
static FAILURE_COUNT: LazyLock<FailureCount> = LazyLock::new(|| FailureCount::new(5));

#[derive(Clone)]
pub struct TelegramBot {
    api: Arc<Api>,
    pool: PgPool,
    cal_sync: Arc<CalendarSync>,
    queue: Arc<Queue<(ChatId, StackString)>>,
}

impl TelegramBot {
    pub async fn new(bot_token: &str, pool: &PgPool, config: &Config) -> Self {
        Self {
            api: Arc::new(Api::new(bot_token)),
            pool: pool.clone(),
            cal_sync: Arc::new(CalendarSync::new(config.clone(), pool.clone()).await),
            queue: Arc::new(Queue::new()),
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
            match Box::pin(timeout(
                std::time::Duration::from_secs(3600),
                self.bot_handler(),
            ))
            .await
            {
                Ok(Ok(())) | Err(_) => FAILURE_COUNT.reset()?,
                Ok(Err(_)) => FAILURE_COUNT.increment()?,
            }
        }
    }

    pub async fn bot_handler(&self) -> Result<(), Error> {
        let mut stream = self.api.stream();
        loop {
            let result = select! {
                Some(update) = stream.next() => {
                    self.process_update(update).await
                },
                (chat, msg) = self.queue.pop() => {
                    self.api.spawn(chat.text(msg.as_str()));
                    Ok(())
                },
                else => break,
            };
            result?;
        }
        Ok(())
    }

    async fn process_update(
        &self,
        update: Result<Update, telegram_bot::Error>,
    ) -> Result<(), Error> {
        FAILURE_COUNT.check()?;
        if let UpdateKind::Message(message) = update?.kind {
            FAILURE_COUNT.check()?;
            if let MessageKind::Text { ref data, .. } = message.kind {
                FAILURE_COUNT.check()?;
                if TELEGRAM_USERIDS.load().contains_key(&message.from.id) {
                    FAILURE_COUNT.check()?;
                    if let ChatRef::Id(chat_id) = message.chat.to_chat_ref() {
                        if data.starts_with("/init") {
                            self.update_telegram_chat_id(message.from.id, chat_id)
                                .await?;
                            let reply = format_sstr!("Initializing chat_id {chat_id}");
                            self.api.send(message.text_reply(reply.as_str())).await?;
                        } else if data.starts_with("/cal") {
                            for event in self.cal_sync.list_agenda(0, 1).await? {
                                self.send_message(
                                    chat_id,
                                    &event
                                        .get_summary(
                                            &self.cal_sync.config.domain,
                                            &self.pool,
                                            &self.cal_sync.config,
                                        )
                                        .await,
                                )?;
                            }
                        }
                    }
                } else {
                    // Answer message with "Hi".
                    let reply = format_sstr!(
                        "Hi, {n}, user_id {i}! You just wrote '{data}'",
                        n = message.from.first_name,
                        i = message.from.id,
                    );
                    self.api.send(message.text_reply(reply.as_str())).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn notification_handler(&self) -> Result<(), Error> {
        let now = OffsetDateTime::now_utc();
        let mut events: VecDeque<_> = self.cal_sync.list_agenda(0, 1).await?.into();
        let mut agenda_datetime = now.date().with_hms(12, 0, 0)?.assume_utc();
        loop {
            FAILURE_COUNT.check()?;
            let now = OffsetDateTime::now_utc();
            for chat_id in TELEGRAM_USERIDS.load().values().flatten() {
                if now > agenda_datetime {
                    agenda_datetime += Duration::days(1);
                    events = self.cal_sync.list_agenda(0, 1).await?.into();
                    for event in &events {
                        self.send_message(
                            *chat_id,
                            &event
                                .get_summary(
                                    &self.cal_sync.config.domain,
                                    &self.pool,
                                    &self.cal_sync.config,
                                )
                                .await,
                        )?;
                    }
                } else {
                    while let Some(event) = events.front() {
                        let start_time: OffsetDateTime = event.start_time.into();
                        if now > start_time - Duration::minutes(5) {
                            self.send_message(
                                *chat_id,
                                &event
                                    .get_summary(
                                        &self.cal_sync.config.domain,
                                        &self.pool,
                                        &self.cal_sync.config,
                                    )
                                    .await,
                            )?;
                            events.pop_front();
                        } else {
                            break;
                        }
                    }
                }
            }
            sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    pub fn send_message(&self, chat: ChatId, msg: &str) -> Result<(), Error> {
        self.queue.push((chat, msg.into()));
        Ok(())
    }

    async fn fill_telegram_user_ids(&self) -> Result<(), Error> {
        loop {
            FAILURE_COUNT.check()?;
            let p = self.pool.clone();
            if let Ok(authorized_users) = AuthorizedUsers::get_authorized_users(&p).await {
                let mut telegram_userids = (*TELEGRAM_USERIDS.load().clone()).clone();
                let mut stream = Box::pin(authorized_users);
                while let Some(user) = stream.try_next().await? {
                    if let Some(userid) = user.telegram_userid {
                        let userid = UserId::new(userid);
                        if !telegram_userids.contains_key(&userid) {
                            telegram_userids.insert(userid, user.telegram_chatid.map(ChatId::new));
                        }
                    }
                }
                TELEGRAM_USERIDS.store(Arc::new(telegram_userids));
                FAILURE_COUNT.reset()?;
            } else {
                FAILURE_COUNT.increment()?;
            }
            sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    async fn update_telegram_chat_id(&self, userid: UserId, chatid: ChatId) -> Result<(), Error> {
        match self.update_telegram_chat_id_impl(userid, chatid).await {
            Ok(()) => FAILURE_COUNT.reset()?,
            Err(_) => FAILURE_COUNT.increment()?,
        }
        Ok(())
    }

    async fn update_telegram_chat_id_impl(
        &self,
        userid: UserId,
        chatid: ChatId,
    ) -> Result<(), Error> {
        let authorized_users: Vec<_> = AuthorizedUsers::get_authorized_users(&self.pool)
            .await?
            .try_filter(|user| future::ready(user.telegram_userid == Some(userid.into())))
            .try_collect()
            .await?;
        for mut user in authorized_users {
            user.telegram_chatid.replace(chatid.into());
            user.update_authorized_users(&self.pool).await?;
            let mut telegram_userids = (*TELEGRAM_USERIDS.load().clone()).clone();
            if let Some(telegram_chatid) = telegram_userids.get_mut(&userid) {
                telegram_chatid.replace(chatid);
            }
        }
        Ok(())
    }
}
