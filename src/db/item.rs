use async_trait::async_trait;
use bson::Document;
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::*;
use mongodb::Database;
use mongodb::{Collection, IndexModel};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error;
use std::io::{Error, ErrorKind};

use crate::trade::transaction_log::AppState;
use crate::trade::BalanceLog;
use crate::trade::TradePosition;

#[async_trait]
pub trait Entity {
    async fn insert(&self, db: &Database) -> Result<(), Box<dyn error::Error>>;
    async fn update(&self, db: &Database) -> Result<(), Box<dyn error::Error>>;
    async fn delete(&self, db: &Database) -> Result<(), Box<dyn error::Error>>;
    async fn delete_all(&self, db: &Database) -> Result<(), Box<dyn error::Error>>;

    async fn search(&self, db: &Database) -> Result<Vec<Self>, Box<dyn error::Error>>
    where
        Self: std::marker::Sized;

    fn get_collection_name(&self) -> &str;

    fn get_collection(&self, db: &Database) -> Collection<Self>
    where
        Self: std::marker::Sized,
    {
        db.collection::<Self>(self.get_collection_name())
    }

    async fn create_unique_index(&self, db: &Database) -> Result<(), Box<dyn error::Error>>
    where
        Self: std::marker::Sized,
        Self: std::marker::Send,
    {
        let options = IndexOptions::builder().unique(true).build();
        let model = IndexModel::builder()
            .keys(doc! {"id": 1})
            .options(options)
            .build();
        let collection = self.get_collection(db);
        collection.create_index(model, None).await?;
        Ok(())
    }
}

pub async fn insert_item<T: Entity>(db: &Database, item: &T) -> Result<(), Box<dyn error::Error>> {
    item.insert(db).await
}

pub async fn update_item<T: Entity>(db: &Database, item: &T) -> Result<(), Box<dyn error::Error>> {
    item.update(db).await
}

pub async fn delete_item<T: Entity>(db: &Database, item: &T) -> Result<(), Box<dyn error::Error>> {
    item.delete(db).await
}

pub async fn delete_item_all<T: Entity>(
    db: &Database,
    item: &T,
) -> Result<(), Box<dyn error::Error>> {
    item.delete_all(db).await
}

pub async fn search_items<T: Entity>(
    db: &Database,
    item: &T,
) -> Result<Vec<T>, Box<dyn error::Error>> {
    item.search(db).await
}

pub async fn search_item<T: Entity>(db: &Database, item: &T) -> Result<T, Box<dyn error::Error>> {
    let mut items = item.search(db).await?;
    if items.len() == 1 {
        Ok(items.pop().unwrap())
    } else {
        Err(Box::new(Error::new(
            ErrorKind::Other,
            "Multiple items are found".to_string(),
        )))
    }
}

pub async fn create_unique_index(db: &Database) -> Result<(), Box<dyn error::Error>> {
    let item = TradePosition::default();
    item.create_unique_index(db).await?;
    Ok(())
}

#[async_trait]
impl Entity for TradePosition {
    async fn insert(&self, db: &Database) -> Result<(), Box<dyn error::Error>> {
        let collection = self.get_collection(db);
        collection.insert_one(self, None).await?;
        Ok(())
    }

    async fn update(&self, db: &Database) -> Result<(), Box<dyn error::Error>> {
        let query = doc! { "id": self.id };
        let update = bson::to_bson(self).unwrap();
        let update = doc! { "$set" : update };
        let collection = self.get_collection(db);
        collection.update(query, update, true).await
    }

    async fn delete(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn delete_all(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn search(&self, db: &Database) -> Result<Vec<Self>, Box<dyn error::Error>> {
        let mut query = doc! { "id": { "$gt": 0 }};
        if self.id != None {
            query = doc! { "id": self.id.unwrap() };
        }
        let collection = self.get_collection(db);
        collection.search(query).await
    }

    fn get_collection_name(&self) -> &str {
        "position"
    }
}

#[async_trait]
impl Entity for BalanceLog {
    async fn insert(&self, db: &Database) -> Result<(), Box<dyn error::Error>> {
        let collection = self.get_collection(db);
        collection.insert_one(self, None).await?;
        Ok(())
    }

    async fn update(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn delete(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn delete_all(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn search(&self, _db: &Database) -> Result<Vec<Self>, Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    fn get_collection_name(&self) -> &str {
        "balance"
    }
}

#[async_trait]
impl Entity for AppState {
    async fn insert(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn update(&self, db: &Database) -> Result<(), Box<dyn error::Error>> {
        let query = doc! { "id": 1 };
        let update = bson::to_bson(self).unwrap();
        let update = doc! { "$set" : update };
        let collection = self.get_collection(db);
        collection.update(query, update, true).await
    }

    async fn delete(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn delete_all(&self, _db: &Database) -> Result<(), Box<dyn error::Error>> {
        panic!("Not implemented")
    }

    async fn search(&self, db: &Database) -> Result<Vec<Self>, Box<dyn error::Error>> {
        let query = doc! { "id": 1 };
        let collection = self.get_collection(db);
        collection.search(query).await
    }

    fn get_collection_name(&self) -> &str {
        "app-state"
    }
}

#[async_trait]
pub trait HelperCollection<T> {
    async fn update(
        &self,
        query: Document,
        update: Document,
        upsert: bool,
    ) -> Result<(), Box<dyn error::Error>>;
    async fn delete(&self, query: Document) -> Result<(), Box<dyn error::Error>>;
    async fn delete_all(&self) -> Result<(), Box<dyn error::Error>>;
    async fn search(&self, query: Document) -> Result<Vec<T>, Box<dyn error::Error>>;
}

#[async_trait]
impl<T> HelperCollection<T> for Collection<T>
where
    T: DeserializeOwned + Unpin + Send + Sync + Serialize + std::fmt::Debug,
{
    async fn update(
        &self,
        query: Document,
        update: Document,
        upsert: bool,
    ) -> Result<(), Box<dyn error::Error>> {
        let options = FindOneAndUpdateOptions::builder()
            .upsert(upsert)
            .return_document(ReturnDocument::After)
            .build();
        let _ = self.find_one_and_update(query, update, options).await?;
        Ok(())
    }

    async fn delete(&self, query: Document) -> Result<(), Box<dyn error::Error>> {
        let result = self.delete_one(query, None).await?;
        if result.deleted_count == 1 {
            return Ok(());
        } else {
            panic!("Not implemented")
        }
    }

    async fn delete_all(&self) -> Result<(), Box<dyn error::Error>> {
        let options = DropCollectionOptions::builder().build();
        self.drop(options).await?;
        Ok(())
    }

    async fn search(&self, query: Document) -> Result<Vec<T>, Box<dyn error::Error>> {
        let find_options = FindOptions::builder().sort(doc! { "id": 1 }).build();
        let mut items: Vec<T> = vec![];
        let mut cursor = self.find(query, find_options).await?;
        while let Some(item) = cursor.try_next().await? {
            items.push(item);
        }
        if items.len() == 0 {
            Err(Box::new(Error::new(
                ErrorKind::Other,
                "Item not found".to_string(),
            )))
        } else {
            Ok(items)
        }
    }
}
