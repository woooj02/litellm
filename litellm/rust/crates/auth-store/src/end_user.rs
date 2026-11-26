//! An end user as the database holds it, in the shape the auth cache holds it.

use std::sync::Arc;

use litellm_redis_store::auth_cache::CachedEndUser;
use litellm_repositories::{EndUserRecord, EndUserRepository};

use crate::{Error, budget::cached_budget, object_permission::cached_end_user_object_permission};

/// The end user as the database holds it, in the shape the auth cache holds it: the row with its
/// budget and object permission.
pub struct EndUserDatabase {
    end_users: Arc<EndUserRepository>,
}

impl EndUserDatabase {
    pub fn new(end_users: Arc<EndUserRepository>) -> Self {
        Self { end_users }
    }

    /// The end user `end_user_id`, or `None` when no row has that id. The budget is the row's own:
    /// `get_end_user_object` caches an end user with no budget of its own with the proxy-wide
    /// default end-user budget nested, and that merge stays with the host that caches it.
    pub async fn end_user(&self, end_user_id: &str) -> Result<Option<CachedEndUser>, Error> {
        let Some(found) = self
            .end_users
            .find_with_budget_and_object_permission(end_user_id)
            .await?
        else {
            return Ok(None);
        };
        cached_end_user(found).map(Some)
    }
}

/// What `LiteLLM_EndUserTable.model_dump(mode="json")` holds for `end_user`, or the column the
/// model would refuse the row for. A budget or object permission the model refuses refuses the
/// whole end user, as `get_end_user_object` then caches nothing for it.
fn cached_end_user(end_user: EndUserRecord) -> Result<CachedEndUser, Error> {
    let EndUserRecord {
        user_id,
        alias,
        spend,
        allowed_model_region,
        default_model,
        models,
        budget_id,
        litellm_budget_table,
        object_permission_id,
        object_permission,
        blocked,
    } = end_user;
    Ok(CachedEndUser {
        user_id,
        blocked,
        alias,
        spend,
        allowed_model_region: allowed_model_region
            .map(|region| {
                region.parse().map_err(|_| Error::Row {
                    column: "allowed_model_region",
                })
            })
            .transpose()?,
        default_model,
        models,
        budget_id,
        litellm_budget_table: litellm_budget_table
            .map(|budget| cached_budget(budget.0))
            .transpose()?,
        object_permission_id,
        object_permission: object_permission
            .map(|permission| cached_end_user_object_permission(permission.0))
            .transpose()?,
        unread: Vec::new(),
    })
}
