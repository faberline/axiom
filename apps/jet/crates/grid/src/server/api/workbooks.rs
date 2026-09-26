use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::db::models::Workbook;
use crate::server::error::AppError;
use crate::server::AppState;

/// Request to create a new workbook
#[derive(Debug, Deserialize)]
pub struct CreateWorkbookRequest {
    pub name: String,
}

/// Response for workbook operations
#[derive(Debug, Serialize)]
pub struct WorkbookResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Workbook> for WorkbookResponse {
    fn from(wb: Workbook) -> Self {
        Self {
            id: wb.id,
            name: wb.name,
            created_at: wb.created_at,
            updated_at: wb.updated_at,
        }
    }
}

/// List all workbooks
async fn list_workbooks(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkbookResponse>>, AppError> {
    let workbooks = state.db.list_workbooks().await?;
    let response: Vec<WorkbookResponse> = workbooks.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

/// Create a new workbook
async fn create_workbook(
    State(state): State<AppState>,
    Json(req): Json<CreateWorkbookRequest>,
) -> Result<Json<WorkbookResponse>, AppError> {
    let workbook = state.db.create_workbook(&req.name).await?;
    Ok(Json(workbook.into()))
}

/// Get a workbook by ID
async fn get_workbook(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkbookResponse>, AppError> {
    let workbook = state
        .db
        .get_workbook(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Workbook {} not found", id)))?;
    Ok(Json(workbook.into()))
}

/// Update a workbook
async fn update_workbook(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateWorkbookRequest>,
) -> Result<Json<WorkbookResponse>, AppError> {
    let workbook = state.db.update_workbook(id, &req.name).await?;
    Ok(Json(workbook.into()))
}

/// Delete a workbook
async fn delete_workbook(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.db.delete_workbook(id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Get workbook content (as JSON)
async fn get_workbook_content(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let content = state.db.get_workbook_content(id).await?;
    match content {
        Some(json) => Ok(Json(json)),
        None => Ok(Json(serde_json::json!({
            "name": "Untitled",
            "sheets": [{"name": "Sheet1", "cells": {}}],
            "activeSheetIndex": 0
        }))),
    }
}

/// Save workbook content (as JSON)
async fn save_workbook_content(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(content): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.db.save_workbook_content(id, &content).await?;
    Ok(Json(serde_json::json!({ "saved": true })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/workbooks", get(list_workbooks).post(create_workbook))
        .route(
            "/api/workbooks/:id",
            get(get_workbook)
                .put(update_workbook)
                .delete(delete_workbook),
        )
        .route(
            "/api/workbooks/:id/content",
            get(get_workbook_content).put(save_workbook_content),
        )
}
