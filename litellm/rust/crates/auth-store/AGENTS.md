# Auth store

Database reads for `gateway-auth-core`'s store ports. `KeyDatabase`, `TeamDatabase`, `ProjectDatabase`, `TeamMembershipDatabase` and `EndUserDatabase` each read an auth object through the `repositories` readers and shape it into the record the auth cache holds for it. Every host of the core reads a key's row through this crate, so no host shapes the row on its own

## Boundary

- No decision lives here. The core decides who may call what, and this crate only reads. A caller that must refuse on what a read found, such as a writer read whose object permission row is gone, gets the facts back and refuses itself
- No cache is touched here. The value is the family's typed record in `redis-store/src/auth_cache/types/`, and reading, writing and evicting the cache stay with the host
- No query is written here. A row comes from its repository
- It covers only objects the core asks a host for. The object permission is read here because a cached key and a cached end user nest it
- Concrete structs, never a trait. A test drives them against real Postgres

## The read

- `KeyDatabase::key` is the key's one query, shaped into `CachedKey` with nothing nested. `KeyDatabase::read` adds the object permission it points at, for a host with no bound of its own to put between the two reads
  - The Python proxy bounds each read on its own: its lookup gate and deadline go around `RustKeyRepository.find`, which is `KeyDatabase::key`. It then nests the permission through its own `get_object_permission`, under that read's own deadline, and caches the key with `cache_key_object`. A permission table that stalls must leave that key served without the permission
- `TeamDatabase::team` is a team's one query, shaped into `CachedTeam` with its model alias table and nothing else nested. The object permission it points at is not read, and `last_refreshed_at` is unset, since the host that caches a team stamps it. Only the gateway reads a team here so far. The Python proxy still shapes its own from the same repository row, so `tests/team.rs`, `redis-store/tests/auth_cache_team.rs` and `tests/unit/redis/stores/test_auth_cache_team.py` hold the same row and entry literals and are changed together, and a field Python adds to `LiteLLM_TeamTableCachedObj` fails there until it is named here
- `ProjectDatabase::project` is a project's one query, shaped into `CachedProject` with its budget and nothing else nested. It is the shape `get_project_object` caches under `project_id:{id}`: the object permission is not read, and `last_refreshed_at` is unset, since the host that caches a project stamps it. `tests/project.rs`, `redis-store/tests/auth_cache_project.rs` and `tests/unit/redis/stores/test_auth_cache_project.py` hold the same row and entry literals and are changed together
  - The budget a project nests is shaped by `budget.rs` into `CachedBudget`, which is `LiteLLM_BudgetTable`'s dump
- `TeamMembershipDatabase::team_membership` is a membership's one query, shaped into `CachedTeamMembership` with its budget. It is the shape the team member budget check caches under `{team_id}_{user_id}`, and `get_team_membership` caches the same dump under `team_membership:{user_id}:{team_id}`. `tests/team_membership.rs`, `redis-store/tests/auth_cache_team_membership.rs` and `tests/unit/redis/stores/test_auth_cache_team_membership.py` hold the same row and entry literals and are changed together
  - The membership types its budget as `LiteLLM_BudgetTableFull | LiteLLM_BudgetTable`, and pydantic reads every database row as the full one, so `budget.rs` shapes it into `CachedBudgetFull`: the `CachedBudget` with `budget_reset_at` and `created_at` after it
  - The auth prefetch writes both keys from the key owners row instead, whose budget times are cached without a `Z` and whose NULL `allowed_models` is cached as `[]`, so its bytes differ from this shape for the same row
- `EndUserDatabase::end_user` is an end user's one query, shaped into `CachedEndUser` with its budget and object permission. It is the shape `get_end_user_object` caches under `end_user_id:{id}`, its only writer to the shared cache. `tests/end_user.rs`, `redis-store/tests/auth_cache_end_user.rs` and `tests/unit/redis/stores/test_auth_cache_end_user.py` hold the same row and entry literals and are changed together
  - The budget is the row's own. `get_end_user_object` caches an end user with no budget of its own with the proxy-wide default end-user budget merged into `litellm_budget_table` and `budget_id` still unset. That merge stays with the host, and `CachedEndUser` carries either budget
  - The budget is `LiteLLM_BudgetTable`, so `budget.rs` shapes it into `CachedBudget`, and a budget with windows refuses the whole end user, as the model does. `get_end_user_object` then caches nothing for it
  - The end user query reads its object permission with `to_jsonb`, so an array column that is SQL NULL is left out of the row and takes the model's default, an empty list but unset for `mcp_toolsets` and `skills`. `object_permission.rs` shapes it with `cached_end_user_object_permission`. A permission the model refuses refuses the whole end user too
- Nothing is returned for an id no row has

## The cached shape

- What the database gives is not what the cache holds. A key is cached as Python's merged `UserAPIKeyAuth` dump, so `key.rs` converts the `combined_view` row field by field into `CachedKey`
- The entry is a wire format Python still owns: `cache_key_object` writes it for every key the proxy reads or a management endpoint changes, and pods on an older release fill it from the database themselves. So the record this crate shapes for a row dumps to the bytes `main`'s `get_key_object` wrote for it. `tests/key.rs`, `redis-store/tests/auth_cache_key.rs` and `tests/unit/redis/stores/test_auth_cache_key.py` hold the same row and entry literals and are changed together, so a field Python adds to `UserAPIKeyAuth` fails there until it is named here
- Nothing the Python model would refuse is returned. A column takes only the shapes its field takes (`metadata` an object, `budget_fallbacks` lists of strings), and a row that breaks one fails the read with the column's name, as the model's validation did
  - The object permission is the exception `get_key_object` made: a permission row that cannot be read or that its model would refuse leaves a replica read's key without it, so one bad permission never takes down a key that inference requests still use. A writer read fails on it
