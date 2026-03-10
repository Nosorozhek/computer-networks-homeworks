use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::{CreateProductRequest, Product, UpdateProductRequest};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
};
use axum::extract::Multipart;
use sqlx::Row;

pub async fn create_product(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateProductRequest>,
) -> Result<Json<Product>, AppError> {
    let id = sqlx::query("INSERT INTO products (name, description) VALUES (?, ?) RETURNING id")
        .bind(&payload.name)
        .bind(&payload.description)
        .fetch_one(&app_state.pool)
        .await?
        .get(0);

    Ok(Json(Product {
        id,
        name: payload.name,
        description: payload.description,
        icon: None,
    }))
}

pub async fn get_product(
    State(app_state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Product>, AppError> {
    let product = sqlx::query_as::<_, Product>(
        "SELECT id, name, description, icon FROM products WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&app_state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(product))
}

pub async fn update_product(
    State(app_state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateProductRequest>,
) -> Result<Json<Product>, AppError> {
    let current = sqlx::query_as::<_, Product>(
        "SELECT id, name, description, icon FROM products WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&app_state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_name = payload.name.unwrap_or(current.name);
    let new_description = payload.description.unwrap_or(current.description);

    sqlx::query("UPDATE products SET name = ?, description = ? WHERE id = ?")
        .bind(&new_name)
        .bind(&new_description)
        .bind(id)
        .execute(&app_state.pool)
        .await?;

    Ok(Json(Product {
        id,
        name: new_name,
        description: new_description,
        icon: current.icon,
    }))
}

pub async fn delete_product(
    State(app_state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Product>, AppError> {
    let deleted_product = sqlx::query_as::<_, Product>(
        "DELETE FROM products WHERE id = ? RETURNING id, name, description, icon",
    )
    .bind(id)
    .fetch_optional(&app_state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let _ = app_state.icon_manager.delete_icon(id);

    Ok(Json(deleted_product))
}

pub async fn get_products(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Product>>, AppError> {
    let products = sqlx::query_as::<_, Product>("SELECT id, name, description, icon FROM products")
        .fetch_all(&app_state.pool)
        .await?;

    Ok(Json(products))
}

pub async fn add_icon(
    State(app_state): State<AppState>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> Result<StatusCode, AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE id = ?)")
        .bind(id)
        .fetch_one(&app_state.pool)
        .await?;

    if !exists {
        return Err(AppError::NotFound);
    }

    while let Some(field) = multipart.next_field().await? {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "icon" {
            let data = field.bytes().await?;
            let icon = app_state.icon_manager.save_icon(id, &data)?;
            sqlx::query("UPDATE products SET icon = ? WHERE id = ?")
                .bind(icon.display().to_string())
                .bind(id)
                .execute(&app_state.pool)
                .await?;
            return Ok(StatusCode::CREATED);
        }
    }

    Err(AppError::BadRequest("Missing 'icon' field in request body".to_string()))
}
pub async fn get_icon(
    State(app_state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let bytes = app_state.icon_manager.get_icon(id)?;

    let content_type = infer::get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    let headers = [(header::CONTENT_TYPE, content_type)];

    Ok((headers, Bytes::from(bytes)))
}
