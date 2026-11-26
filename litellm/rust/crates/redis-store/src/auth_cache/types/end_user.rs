use litellm_python_compat::Value;

use super::{
    budget::CachedBudget,
    object_permission::CachedObjectPermission,
    record::{Field, fields, record, required},
};
use crate::{DecodeError, FromValue, IntoValue};

/// The two regions `LiteLLM_EndUserTable.allowed_model_region` takes.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, strum::EnumString, strum::IntoStaticStr, strum::VariantArray,
)]
pub enum ModelRegion {
    #[strum(serialize = "eu")]
    Eu,
    #[strum(serialize = "us")]
    Us,
}

impl Field for ModelRegion {
    fn read(value: &Value) -> Option<Self> {
        match value {
            Value::Str(region) => region.parse().ok(),
            _ => None,
        }
    }

    fn write(self) -> Value {
        Value::from(<&'static str>::from(self))
    }
}

record! {
    /// `LiteLLM_EndUserTable.model_dump(mode="json")`, the value `get_end_user_object` caches under
    /// [`AuthObject::EndUser`](crate::auth_cache::AuthObject::EndUser), with the budget and object
    /// permission nested. An end user with no budget of its own is cached with the proxy-wide
    /// default end-user budget in `litellm_budget_table` and its own `budget_id` still `None`, so
    /// the budget an entry holds is not always the one the row names. Nothing is cached for an id
    /// no row has or a row the model refuses.
    pub struct CachedEndUser {
        user_id: String = required!("user_id"),
        blocked: bool = required!("blocked"),
        alias: Option<String>,
        spend: f64,
        allowed_model_region: Option<ModelRegion>,
        default_model: Option<String>,
        models: Vec<String>,
        budget_id: Option<String>,
        litellm_budget_table: Option<CachedBudget>,
        object_permission_id: Option<String>,
        object_permission: Option<CachedObjectPermission>,
    }
}
