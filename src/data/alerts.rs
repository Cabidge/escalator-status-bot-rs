use itertools::Itertools;
use shuttle_persist::PersistInstance;

use crate::prelude::*;

use std::collections::HashMap;

use super::{escalator_input::EscalatorInput, status::Status, ESCALATORS, ESCALATOR_COUNT};

#[derive(Debug)]
pub struct Alerts {
    watch_lists: HashMap<serenity::User, [bool; ESCALATOR_COUNT]>,
    should_save: bool,
}

type WatchList = [bool; ESCALATOR_COUNT];
type SimplifiedWatchLists = Vec<(u64, WatchList)>;

impl Alerts {
    pub async fn load_persist(
        persist: &PersistInstance,
        cache_http: impl serenity::CacheHttp,
    ) -> Self {
        let Ok(simplified) = persist.load::<SimplifiedWatchLists>("alerts") else {
            return Alerts {
                watch_lists: HashMap::new(),
                should_save: true,
            };
        };

        let mut watch_lists = HashMap::new();
        let mut should_save = false;
        for (user_id, watch_list) in simplified {
            let Ok(user) = serenity::UserId(user_id).to_user(&cache_http).await else {
                should_save = true;
                continue;
            };

            watch_lists.insert(user, watch_list);
        }

        Alerts {
            watch_lists,
            should_save,
        }
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        match self.should_save {
            true => self.should_save = false,
            false => return,
        }

        let simplified = self
            .watch_lists
            .iter()
            .map(|(user, watch_list)| (user.id.0, watch_list))
            .collect_vec();

        persist.save("alerts", simplified).ok();
    }

    // Replace a user's original watch list with an updated list.
    pub fn replace(&mut self, user: &serenity::User, watch_list: WatchList) {
        // if the watch list is all unselected
        if watch_list.iter().all(|selected| !selected) {
            // remove the watch list
            if self.watch_lists.remove(user).is_some() {
                // if the user had a watch list originally, mark as should save
                self.should_save = true;
            }
            return;
        }

        let Some(original_list) = self.watch_lists.get_mut(user) else {
            // if there was no list before, add a watch list for the user
            // and mark as should save
            self.watch_lists.insert(user.to_owned(), watch_list);
            self.should_save = true;
            return;
        };

        // if the new watch list is different from the original
        if original_list != &watch_list {
            // replace the watch list and mark as should save
            *original_list = watch_list;
            self.should_save = true;
        }
    }

    pub fn get_watch_list(&self, user: &serenity::User) -> Option<WatchList> {
        self.watch_lists.get(user).copied()
    }

    pub async fn alert(
        &self,
        cache_http: impl serenity::CacheHttp,
        escalators: EscalatorInput,
        status: Status,
    ) {
        let Ok(users) = self.users_watching(escalators) else {
            return
        };

        let emoji = status.emoji();
        let noun = escalators.message_noun();
        let is_are = if escalators.is_singular() {
            "is"
        } else {
            "are"
        };
        let status = status.as_id_str();
        let message = format!("`{emoji}` {noun} {is_are} `{status}`");

        for user in users {
            user.direct_message(&cache_http, |msg| msg.content(&message))
                .await
                .ok();
        }
    }

    fn users_watching(
        &self,
        escalators: EscalatorInput,
    ) -> Result<impl Iterator<Item = &serenity::User>, UnknownEscalatorError> {
        let escalators = EscalatorIndex::try_from(escalators)?;

        Ok(self
            .watch_lists
            .iter()
            .filter_map(move |(user, watch_list)| {
                is_on_watch_list(watch_list, escalators).then_some(user)
            }))
    }
}

#[derive(Clone, Copy)]
enum EscalatorIndex {
    All,
    Direct(usize),
    Pair(usize, usize),
}

fn escalator_index(escalator: Escalator) -> Option<usize> {
    ESCALATORS.into_iter().position(|e| e == escalator)
}

struct UnknownEscalatorError;

impl TryFrom<EscalatorInput> for EscalatorIndex {
    type Error = UnknownEscalatorError;

    fn try_from(value: EscalatorInput) -> Result<Self, Self::Error> {
        match value {
            EscalatorInput::All => Ok(Self::All),
            EscalatorInput::Direct(start, end) => escalator_index((start, end))
                .ok_or(UnknownEscalatorError)
                .map(Self::Direct),
            EscalatorInput::Pair(a, b) => {
                let forward = escalator_index((a, b)).ok_or(UnknownEscalatorError)?;
                let reverse = escalator_index((b, a)).ok_or(UnknownEscalatorError)?;
                Ok(Self::Pair(forward, reverse))
            }
        }
    }
}

fn is_on_watch_list(watch_list: &WatchList, escalators: EscalatorIndex) -> bool {
    match escalators {
        EscalatorIndex::All => !watch_list.is_empty(),
        EscalatorIndex::Direct(index) => watch_list[index],
        EscalatorIndex::Pair(forward, reverse) => watch_list[forward] || watch_list[reverse],
    }
}
