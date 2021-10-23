use crate::{get_creds_and_project, model::Channel};
use async_trait::async_trait;
use deadpool::managed;
use std::convert::Infallible;
use tiny_firestore_odm::{Collection, Database};

pub struct NotifyDatabase {
    db: Database,
}

impl NotifyDatabase {
    pub fn channels(&self) -> Collection<Channel> {
        self.db.collection("channels")
    }
}

pub struct NotifyDatabaseManager;

#[async_trait]
impl managed::Manager for NotifyDatabaseManager {
    type Type = NotifyDatabase;
    type Error = Infallible;

    async fn create(&self) -> Result<NotifyDatabase, Infallible> {
        let (token_source, project_id) = get_creds_and_project().await;
        let db = Database::new(token_source, &project_id).await;

        Ok(NotifyDatabase { db })
    }

    async fn recycle(&self, _: &mut NotifyDatabase) -> managed::RecycleResult<Infallible> {
        Ok(())
    }
}
