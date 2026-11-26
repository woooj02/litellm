//! An object permission as the auth cache holds it, nested in the key or end user that points at it.

use litellm_redis_store::auth_cache::CachedObjectPermission;
use litellm_repositories::{EndUserObjectPermission, ObjectPermission};

use crate::{Error, column::nullable};

/// What `LiteLLM_ObjectPermissionTable.model_dump(mode="json")` holds for `row`, or the column the
/// model would refuse the row for. The table has no `apps` column, so the model's default stands.
pub(crate) fn cached_object_permission(
    row: ObjectPermission,
) -> Result<CachedObjectPermission, Error> {
    let ObjectPermission {
        object_permission_id,
        mcp_servers,
        mcp_access_groups,
        mcp_tool_permissions,
        vector_stores,
        agents,
        agent_access_groups,
        models,
        blocked_tools,
        mcp_toolsets,
        search_tools,
        mcp_tool_search_enabled,
        skills,
    } = row;
    Ok(CachedObjectPermission {
        object_permission_id,
        mcp_servers: Some(mcp_servers),
        mcp_access_groups: Some(mcp_access_groups),
        mcp_tool_permissions: nullable("mcp_tool_permissions", mcp_tool_permissions.as_ref())?,
        vector_stores: Some(vector_stores),
        agents: Some(agents),
        agent_access_groups: Some(agent_access_groups),
        models: Some(models),
        mcp_toolsets: Some(mcp_toolsets),
        blocked_tools: Some(blocked_tools),
        search_tools: Some(search_tools),
        mcp_tool_search_enabled,
        skills: Some(skills),
        apps: None,
        unread: Vec::new(),
    })
}

/// [`cached_object_permission`] for the permission an end user's row nests, whose array columns
/// are `None` when SQL NULL. The repository leaves those out of the row it hands Python, so each
/// takes the model's default: an empty list, but unset for `mcp_toolsets` and `skills`.
pub(crate) fn cached_end_user_object_permission(
    row: EndUserObjectPermission,
) -> Result<CachedObjectPermission, Error> {
    let EndUserObjectPermission {
        object_permission_id,
        mcp_servers,
        mcp_access_groups,
        mcp_tool_permissions,
        vector_stores,
        agents,
        agent_access_groups,
        models,
        blocked_tools,
        mcp_toolsets,
        search_tools,
        mcp_tool_search_enabled,
        skills,
    } = row;
    Ok(CachedObjectPermission {
        object_permission_id,
        mcp_servers: Some(mcp_servers.unwrap_or_default()),
        mcp_access_groups: Some(mcp_access_groups.unwrap_or_default()),
        mcp_tool_permissions: nullable("mcp_tool_permissions", mcp_tool_permissions.as_ref())?,
        vector_stores: Some(vector_stores.unwrap_or_default()),
        agents: Some(agents.unwrap_or_default()),
        agent_access_groups: Some(agent_access_groups.unwrap_or_default()),
        models: Some(models.unwrap_or_default()),
        mcp_toolsets,
        blocked_tools: Some(blocked_tools.unwrap_or_default()),
        search_tools: Some(search_tools.unwrap_or_default()),
        mcp_tool_search_enabled,
        skills,
        apps: None,
        unread: Vec::new(),
    })
}
