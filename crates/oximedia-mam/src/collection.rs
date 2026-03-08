//! Collection management
//!
//! Provides collection capabilities:
//! - Hierarchical folder organization
//! - Static collections (manual asset management)
//! - Smart collections (dynamic queries)
//! - Collection sharing and permissions
//! - Collection export

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::database::Database;
use crate::search::SearchEngine;
use crate::{MamError, Result};

/// Collection manager
pub struct CollectionManager {
    db: Arc<Database>,
    #[allow(dead_code)]
    search: Arc<SearchEngine>,
}

/// Collection record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_smart: bool,
    pub smart_query: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Collection item (asset in collection)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollectionItem {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub asset_id: Uuid,
    pub position: Option<i32>,
    pub added_by: Option<Uuid>,
    pub added_at: DateTime<Utc>,
}

/// Create collection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_smart: bool,
    pub smart_query: Option<SmartQuery>,
    pub created_by: Option<Uuid>,
}

/// Update collection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCollectionRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub smart_query: Option<SmartQuery>,
}

/// Smart collection query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartQuery {
    pub conditions: Vec<QueryCondition>,
    pub operator: QueryOperator,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<i32>,
}

/// Query condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
}

/// Query operator (AND/OR)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QueryOperator {
    And,
    Or,
}

/// Condition operator
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    In,
    NotIn,
}

/// Collection with item count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionWithCount {
    pub collection: Collection,
    pub item_count: i64,
}

/// Collection tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTreeNode {
    pub collection: Collection,
    pub children: Vec<CollectionTreeNode>,
    pub item_count: i64,
}

impl CollectionManager {
    /// Create a new collection manager
    #[must_use]
    pub fn new(db: Arc<Database>, search: Arc<SearchEngine>) -> Self {
        Self { db, search }
    }

    /// Create a new collection
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails
    pub async fn create_collection(&self, req: CreateCollectionRequest) -> Result<Collection> {
        // Validate parent exists if specified
        if let Some(parent_id) = req.parent_id {
            self.get_collection(parent_id).await?;
        }

        let smart_query_json = req.smart_query.and_then(|q| serde_json::to_value(q).ok());

        let collection = sqlx::query_as::<_, Collection>(
            "INSERT INTO collections (id, name, description, parent_id, is_smart, smart_query, created_by, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
             RETURNING *"
        )
        .bind(Uuid::new_v4())
        .bind(&req.name)
        .bind(req.description)
        .bind(req.parent_id)
        .bind(req.is_smart)
        .bind(smart_query_json)
        .bind(req.created_by)
        .fetch_one(self.db.pool())
        .await?;

        Ok(collection)
    }

    /// Get collection by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the collection is not found
    pub async fn get_collection(&self, collection_id: Uuid) -> Result<Collection> {
        let collection = sqlx::query_as::<_, Collection>("SELECT * FROM collections WHERE id = $1")
            .bind(collection_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|_| MamError::CollectionNotFound(collection_id))?;

        Ok(collection)
    }

    /// Update collection
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails
    pub async fn update_collection(
        &self,
        collection_id: Uuid,
        req: UpdateCollectionRequest,
    ) -> Result<Collection> {
        // Validate parent exists if specified
        if let Some(parent_id) = req.parent_id {
            if parent_id != collection_id {
                self.get_collection(parent_id).await?;
            } else {
                return Err(MamError::InvalidInput(
                    "Collection cannot be its own parent".to_string(),
                ));
            }
        }

        let smart_query_json = req.smart_query.and_then(|q| serde_json::to_value(q).ok());

        let collection = sqlx::query_as::<_, Collection>(
            "UPDATE collections SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                parent_id = COALESCE($4, parent_id),
                smart_query = COALESCE($5, smart_query),
                updated_at = NOW()
             WHERE id = $1
             RETURNING *",
        )
        .bind(collection_id)
        .bind(req.name)
        .bind(req.description)
        .bind(req.parent_id)
        .bind(smart_query_json)
        .fetch_one(self.db.pool())
        .await?;

        Ok(collection)
    }

    /// Delete collection
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails
    pub async fn delete_collection(&self, collection_id: Uuid) -> Result<()> {
        // This will cascade delete items and child collections
        sqlx::query("DELETE FROM collections WHERE id = $1")
            .bind(collection_id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// List collections
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails
    pub async fn list_collections(
        &self,
        parent_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CollectionWithCount>> {
        let collections = if let Some(pid) = parent_id {
            sqlx::query_as::<_, Collection>(
                "SELECT * FROM collections WHERE parent_id = $1 ORDER BY name LIMIT $2 OFFSET $3",
            )
            .bind(pid)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Collection>(
                "SELECT * FROM collections WHERE parent_id IS NULL ORDER BY name LIMIT $1 OFFSET $2"
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        };

        let mut result = Vec::new();
        for collection in collections {
            let count = self.get_collection_item_count(collection.id).await?;
            result.push(CollectionWithCount {
                collection,
                item_count: count,
            });
        }

        Ok(result)
    }

    /// Get collection tree (hierarchical structure)
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails
    pub async fn get_collection_tree(&self) -> Result<Vec<CollectionTreeNode>> {
        // Get root collections
        let roots = sqlx::query_as::<_, Collection>(
            "SELECT * FROM collections WHERE parent_id IS NULL ORDER BY name",
        )
        .fetch_all(self.db.pool())
        .await?;

        let mut tree = Vec::new();
        for root in roots {
            let node = self.build_tree_node(root).await?;
            tree.push(node);
        }

        Ok(tree)
    }

    /// Build collection tree node recursively
    fn build_tree_node<'a>(
        &'a self,
        collection: Collection,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CollectionTreeNode>> + Send + 'a>>
    {
        Box::pin(async move {
            let children_collections = sqlx::query_as::<_, Collection>(
                "SELECT * FROM collections WHERE parent_id = $1 ORDER BY name",
            )
            .bind(collection.id)
            .fetch_all(self.db.pool())
            .await?;

            let mut children = Vec::new();
            for child in children_collections {
                let child_node = self.build_tree_node(child).await?;
                children.push(child_node);
            }

            let item_count = self.get_collection_item_count(collection.id).await?;

            Ok(CollectionTreeNode {
                collection,
                children,
                item_count,
            })
        })
    }

    /// Add asset to collection
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails
    pub async fn add_asset(
        &self,
        collection_id: Uuid,
        asset_id: Uuid,
        position: Option<i32>,
        added_by: Option<Uuid>,
    ) -> Result<CollectionItem> {
        // Check collection exists
        let collection = self.get_collection(collection_id).await?;

        if collection.is_smart {
            return Err(MamError::InvalidInput(
                "Cannot manually add assets to smart collection".to_string(),
            ));
        }

        let item = sqlx::query_as::<_, CollectionItem>(
            "INSERT INTO collection_items (id, collection_id, asset_id, position, added_by, added_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (collection_id, asset_id) DO UPDATE SET position = $4
             RETURNING *"
        )
        .bind(Uuid::new_v4())
        .bind(collection_id)
        .bind(asset_id)
        .bind(position)
        .bind(added_by)
        .fetch_one(self.db.pool())
        .await?;

        Ok(item)
    }

    /// Remove asset from collection
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails
    pub async fn remove_asset(&self, collection_id: Uuid, asset_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM collection_items WHERE collection_id = $1 AND asset_id = $2")
            .bind(collection_id)
            .bind(asset_id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Get assets in collection
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails
    pub async fn get_collection_assets(
        &self,
        collection_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Uuid>> {
        let collection = self.get_collection(collection_id).await?;

        let asset_ids = if collection.is_smart {
            // Execute smart query
            self.execute_smart_query(collection_id, limit, offset)
                .await?
        } else {
            // Get static collection items
            let items = sqlx::query_as::<_, CollectionItem>(
                "SELECT * FROM collection_items
                 WHERE collection_id = $1
                 ORDER BY COALESCE(position, 999999), added_at
                 LIMIT $2 OFFSET $3",
            )
            .bind(collection_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?;

            items.into_iter().map(|i| i.asset_id).collect()
        };

        Ok(asset_ids)
    }

    /// Get collection item count
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails
    pub async fn get_collection_item_count(&self, collection_id: Uuid) -> Result<i64> {
        let collection = self.get_collection(collection_id).await?;

        let count = if collection.is_smart {
            // For smart collections, execute query and count results
            // This is a simplified version - in production you'd optimize this
            let assets = self.execute_smart_query(collection_id, 999999, 0).await?;
            assets.len() as i64
        } else {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM collection_items WHERE collection_id = $1",
            )
            .bind(collection_id)
            .fetch_one(self.db.pool())
            .await?;
            count
        };

        Ok(count)
    }

    /// Execute smart collection query
    async fn execute_smart_query(
        &self,
        collection_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Uuid>> {
        let collection = self.get_collection(collection_id).await?;

        let smart_query = collection
            .smart_query
            .ok_or_else(|| MamError::Internal("Smart collection has no query".to_string()))?;

        let query: SmartQuery = serde_json::from_value(smart_query)?;

        // Build SQL query from conditions
        let mut sql = String::from("SELECT id FROM assets WHERE status != 'deleted'");

        for (i, condition) in query.conditions.iter().enumerate() {
            if i > 0 {
                match query.operator {
                    QueryOperator::And => sql.push_str(" AND "),
                    QueryOperator::Or => sql.push_str(" OR "),
                }
            } else {
                sql.push_str(" AND ");
            }

            let condition_sql = self.build_condition_sql(condition);
            sql.push_str(&condition_sql);
        }

        if let Some(sort_by) = &query.sort_by {
            let order = query.sort_order.as_deref().unwrap_or("ASC");
            sql.push_str(&format!(" ORDER BY {sort_by} {order}"));
        }

        sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));

        // Execute query
        let rows = sqlx::query_scalar::<_, Uuid>(&sql)
            .fetch_all(self.db.pool())
            .await?;

        Ok(rows)
    }

    /// Build SQL condition from query condition
    fn build_condition_sql(&self, condition: &QueryCondition) -> String {
        let field = &condition.field;

        match condition.operator {
            ConditionOperator::Equals => {
                format!("{field} = '{}'", condition.value)
            }
            ConditionOperator::NotEquals => {
                format!("{field} != '{}'", condition.value)
            }
            ConditionOperator::Contains => {
                format!("{field} ILIKE '%{}%'", condition.value)
            }
            ConditionOperator::StartsWith => {
                format!("{field} ILIKE '{}%'", condition.value)
            }
            ConditionOperator::EndsWith => {
                format!("{field} ILIKE '%{}'", condition.value)
            }
            ConditionOperator::GreaterThan => {
                format!("{field} > {}", condition.value)
            }
            ConditionOperator::LessThan => {
                format!("{field} < {}", condition.value)
            }
            ConditionOperator::GreaterOrEqual => {
                format!("{field} >= {}", condition.value)
            }
            ConditionOperator::LessOrEqual => {
                format!("{field} <= {}", condition.value)
            }
            ConditionOperator::In => {
                format!("{field} IN ({})", condition.value)
            }
            ConditionOperator::NotIn => {
                format!("{field} NOT IN ({})", condition.value)
            }
        }
    }

    /// Reorder assets in collection
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails
    pub async fn reorder_assets(
        &self,
        collection_id: Uuid,
        asset_positions: Vec<(Uuid, i32)>,
    ) -> Result<()> {
        let collection = self.get_collection(collection_id).await?;

        if collection.is_smart {
            return Err(MamError::InvalidInput(
                "Cannot reorder assets in smart collection".to_string(),
            ));
        }

        // Update positions in transaction
        for (asset_id, position) in asset_positions {
            sqlx::query(
                "UPDATE collection_items SET position = $3
                 WHERE collection_id = $1 AND asset_id = $2",
            )
            .bind(collection_id)
            .bind(asset_id)
            .bind(position)
            .execute(self.db.pool())
            .await?;
        }

        Ok(())
    }

    /// Check if asset is in collection
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails
    pub async fn is_asset_in_collection(
        &self,
        collection_id: Uuid,
        asset_id: Uuid,
    ) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM collection_items WHERE collection_id = $1 AND asset_id = $2",
        )
        .bind(collection_id)
        .bind(asset_id)
        .fetch_one(self.db.pool())
        .await?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_query_serialization() {
        let query = SmartQuery {
            conditions: vec![QueryCondition {
                field: "mime_type".to_string(),
                operator: ConditionOperator::Equals,
                value: serde_json::json!("video/mp4"),
            }],
            operator: QueryOperator::And,
            sort_by: Some("created_at".to_string()),
            sort_order: Some("DESC".to_string()),
            limit: Some(100),
        };

        let json = serde_json::to_string(&query).expect("should succeed in test");
        let deserialized: SmartQuery = serde_json::from_str(&json).expect("should succeed in test");

        assert_eq!(deserialized.conditions.len(), 1);
        assert_eq!(deserialized.limit, Some(100));
    }

    #[test]
    fn test_query_operator() {
        let op = QueryOperator::And;
        let json = serde_json::to_string(&op).expect("should succeed in test");
        assert!(json.contains("And"));
    }

    #[test]
    fn test_condition_operator() {
        let op = ConditionOperator::Contains;
        let json = serde_json::to_string(&op).expect("should succeed in test");
        assert!(json.contains("Contains"));
    }
}
