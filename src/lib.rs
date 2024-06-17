/* 2023 (c) | SERGAZIN SOFTWARE
 * MongoDB Methods for rust models
 * Implementing basic CRUD operations for rust models
 * using MongoDB as a backend.
 * Just implement the ModelMethods trait for your model
 * and you are good to go.
 *
 * Depencdenices installation command:
 * cargo add async-trait futures mongodb serde bson
*/

use futures::TryStreamExt;

#[derive(Debug)]
pub enum Error {
    NotFound,
    DBError(mongodb::error::Error),
    BSONSerError(bson::ser::Error),
    CreateFailed(String),
    UpdateFailed(String),
    DeleteFailed(String),
}


#[cfg(feature = "oid_as_id")]
type IdType = bson::oid::ObjectId;
#[cfg(feature = "uuid_as_id")]
type IdType = uuid::Uuid;

#[async_trait::async_trait]
pub trait RustMongoDBModelMethods<E>
where
    Self: serde::ser::Serialize + serde::de::DeserializeOwned + Send + Sync + Unpin + 'static,
    E: From<Error>,
{

    // Implement these methods for your model, that's it!
    fn collection() -> mongodb::Collection<Self>;
    fn id_value(&self) -> &IdType;

    fn id_fitter(id: &IdType) -> bson::Bson {
        #[cfg(feature = "oid_as_id")]
        return id;
        #[cfg(feature = "uuid_as_id")]
        {
            let id = bson::Bson::Binary(bson::Binary{
                subtype:bson::spec::BinarySubtype::Generic,
                bytes: id.as_bytes().to_vec()
            });

            return id;
        }
    }


    // HELPERS =====================================================================================================
    fn search_filter(&self) -> bson::Document {
        #[cfg(feature = "oid_as_id")]
        bson::doc! { "_id": self.id_value() }
        #[cfg(feature = "uuid_as_id")]
        {
            let id = Self::id_fitter(self.id_value());
            bson::doc! { "_id": id }
        }
    }

    // FIND ========================================================================================================
    async fn find(filter: bson::Document) -> Result<Vec<Self>, E> {
        let items = Self::collection()
            .find(filter, None)
            .await
            .map_err(|x| (Error::DBError(x)))?
            .try_collect::<Vec<Self>>()
            .await
            .map_err(|x| (Error::DBError(x)))?;

        Ok(items)
    }

    async fn find_one(filter: bson::Document) -> Result<Option<Self>, E> {
        let item = Self::collection().find_one(filter, None).await.map_err(|x| (Error::DBError(x)))?;
        Ok(item)
    }

    async fn find_one_strict(filter: bson::Document) -> Result<Self, E> {
        let item = Self::find_one(filter).await?.ok_or(Error::NotFound.into())?;
        Ok(item)
    }

    async fn find_by_id(id: &IdType) -> Result<Option<Self>, E> {
        Self::find_one(bson::doc! { "_id": Self::id_fitter(id) }).await
    }
    async fn find_by_id_strict(id: &IdType) -> Result<Self, E> {
        println!("🔑 Finding by ID: {:?}", bson::doc! { "_id": Self::id_fitter(id) });
        Self::find_one_strict(bson::doc! { "_id": Self::id_fitter(id) }).await
    }
    // CREATE ======================================================================================================
    async fn create_one(data: &Self) -> Result<Self, E> {
        let collection = Self::collection();

        let insert_result = collection.insert_one(data, None).await.map_err(|x| (Error::DBError(x)))?;

        #[cfg(feature = "oid_as_id")]
        let some_id = insert_result.inserted_id.as_object_id();
        #[cfg(feature = "uuid_as_id")]
        let some_id: Option<uuid::Uuid> = {
           match insert_result.inserted_id {
               bson::Bson::Binary(bin) => {
                   let uuid = uuid::Uuid::from_slice(&bin.bytes).unwrap();
                   Some(uuid)
               },
               _ => None,
           }
        };

        println!("🔑 Created ID: {:?}", some_id);
        match some_id {
            Some(id) => Ok(Self::find_by_id_strict(&id).await?),
            None => Err(Error::CreateFailed("No ID returned".to_string()).into()),
        }
    }

    // UPDATE ======================================================================================================
    async fn update_one<D: serde::Serialize + Send>(filter: bson::Document, data: D) -> Result<Self, E> {
        let collection = Self::collection();

        let set = bson::to_bson(&data).map_err(|x| (Error::BSONSerError(x)))?;

        let update_result = collection
            .update_one(filter.clone(), bson::doc! { "$set": set }, None)
            .await
            .map_err(|x| (Error::DBError(x)))?;

        if update_result.modified_count != 1 {
            return Err(Error::UpdateFailed("No record updated".to_string()).into());
        };

        Self::find_one_strict(filter).await
    }

    async fn update_by_id<D: serde::Serialize + Send>(id: &IdType, data: D) -> Result<Self, E> {
        Self::update_one(bson::doc! { "_id": Self::id_fitter(id) }, data).await
    }

    // DELETE ======================================================================================================
    async fn delete_one(filter: bson::Document) -> Result<(), E> {
        let collection = Self::collection();

        let delete_result = collection.delete_one(filter, None).await.map_err(|x| (Error::DBError(x)))?;

        if delete_result.deleted_count != 1 {
            return Err(Error::DeleteFailed("No record deleted".to_string()).into());
        };

        Ok(())
    }

    async fn delete_by_id(id: &IdType) -> Result<(), E> {
        Self::delete_one(bson::doc! { "_id": Self::id_fitter(id) }).await
    }

    // Instance Methods
    async fn create(&self) -> Result<Self, E> {
        Self::create_one(self).await
    }
    async fn update<D: serde::Serialize + Send>(&self, data: D) -> Result<Self, E> {
        Self::update_by_id(self.id_value(), data).await
    }
    async fn delete(&self) -> Result<(), E> {
        Self::delete_by_id(self.id_value()).await
    }
}
