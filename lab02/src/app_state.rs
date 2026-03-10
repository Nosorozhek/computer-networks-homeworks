use crate::icon_manager::IconManager;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub icon_manager: IconManager,
}
