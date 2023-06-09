use super::Claims;
use crate::{error::Result, state::AppState};
use axum::{
    body::Empty,
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use axum_extra::extract::Query;
use serde::Deserialize;
use service::sys_dept::{self, DeptCreateParams, DeptUpdateParams};
use utils::extracts::ValidatorJson;
use validator::Validate;

pub fn routers<S>(state: crate::state::AppState) -> axum::Router<S> {
    Router::new()
        .route("/dept", get(index))
        .route("/dept/:id", get(info))
        .route("/dept", post(create))
        .route("/dept/:id", put(update))
        .route("/dept/:id", delete(del))
        .with_state(state)
}
/// 获取树形列表
async fn index(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<SearchRequest>,
) -> Result<impl IntoResponse> {
    Ok(Json(
        sys_dept::get_user_dept_trees(&state.db, claims.user_id, &params.into()).await?,
    ))
}

/// 获取菜单详情
async fn info(Path(id): Path<i32>, State(state): State<AppState>) -> Result<impl IntoResponse> {
    Ok(Json(sys_dept::info(&state.db, id).await?))
}

/// 新增
async fn create(
    State(state): State<AppState>,
    ValidatorJson(params): ValidatorJson<CreateRequest>,
) -> Result<impl IntoResponse> {
    sys_dept::create(&state.db, &params.name.clone(), params.into()).await?;
    Ok(Empty::new())
}

/// 更新
async fn update(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    ValidatorJson(params): ValidatorJson<CreateRequest>,
) -> Result<impl IntoResponse> {
    sys_dept::update(&state.db, id, params.into()).await?;
    Ok(Empty::new())
}

async fn del(Path(id): Path<i32>, State(state): State<AppState>) -> Result<impl IntoResponse> {
    sys_dept::delete(&state.db, id).await?;
    Ok(Empty::new())
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    keyword: Option<String>,
    status: Option<bool>,
}
impl From<SearchRequest> for sys_dept::DeptSearchParams {
    fn from(value: SearchRequest) -> Self {
        Self::new(value.keyword, value.status)
    }
}
#[derive(Debug, Deserialize, Validate)]
struct CreateRequest {
    parent_id: i32,
    name: String,
    person_name: Option<String>,
    person_phone: Option<String>,
    person_email: Option<String>,
    describe: Option<String>,
    status: bool,
    sort: i32,
}

impl From<CreateRequest> for DeptCreateParams {
    fn from(value: CreateRequest) -> Self {
        Self {
            parent_id: Some(value.parent_id),
            person_name: value.person_name,
            person_phone: value.person_phone,
            person_email: value.person_email,
            describe: value.describe,
            status: Some(value.status),
            sort: Some(value.sort),
        }
    }
}
impl From<CreateRequest> for DeptUpdateParams {
    fn from(value: CreateRequest) -> Self {
        Self {
            parent_id: Some(value.parent_id),
            name: Some(value.name),
            person_name: value.person_name,
            person_phone: value.person_phone,
            person_email: value.person_email,
            describe: value.describe,
            status: Some(value.status),
            sort: Some(value.sort),
        }
    }
}
