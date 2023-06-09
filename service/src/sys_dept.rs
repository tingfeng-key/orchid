use crate::{
    prisma::{system_dept, SortOrder},
    sys_role, sys_user, Database, Result, ServiceError,
};
use prisma_client_rust::or;
use serde::Serialize;
use utils::{
    datetime::{now_time, to_local_string},
    tree::{get_tree_start_parent_id, vec_to_tree_into, Tree, TreeInfo},
};

pub async fn create(
    db: &Database,
    name: &str,
    params: DeptCreateParams,
) -> Result<system_dept::Data> {
    Ok(db
        .client
        .system_dept()
        .create_unchecked(name.to_owned(), params.to_params())
        .exec()
        .await?)
}

pub async fn update(db: &Database, id: i32, params: DeptUpdateParams) -> Result<system_dept::Data> {
    Ok(db
        .client
        .system_dept()
        .update_unchecked(system_dept::id::equals(id), params.to_params())
        .exec()
        .await?)
}

pub async fn delete(db: &Database, id: i32) -> Result<system_dept::Data> {
    let res = db
        .client
        ._transaction()
        .run::<ServiceError, _, _, _>(|client| async move {
            let client = std::sync::Arc::new(client);
            let info = client
                .system_dept()
                .update(
                    system_dept::id::equals(id),
                    vec![system_dept::deleted_at::set(Some(now_time()))],
                )
                .exec()
                .await?;
            let user_ids = sys_user::get_users_by_dept_id(db, id)
                .await?
                .into_iter()
                .map(|x| x.get_id())
                .collect::<Vec<i32>>();
            if !user_ids.is_empty() {
                sys_user::batch_set_dept(db, None, user_ids).await?;
            }
            Ok(info)
        })
        .await?;
    Ok(res)
}

pub async fn get_dept_children_ids(db: &Database, parent_dept_id: i32) -> Result<Vec<i32>> {
    let infos = get_dept_tree(
        db,
        &DeptSearchParams {
            keyword: None,
            status: None,
        },
    )
    .await?;
    let mut parent_dept_ids = vec![parent_dept_id];
    Ok(get_children_ids(infos, &mut parent_dept_ids).clone())
}

fn get_children_ids(tree: Vec<Dept>, parent_dept_ids: &mut Vec<i32>) -> &mut Vec<i32> {
    for dept in tree {
        if parent_dept_ids.contains(&dept.info.parent_id) {
            parent_dept_ids.push(dept.info.id);
        }
        if !dept.children.is_empty() {
            get_children_ids(dept.children, parent_dept_ids);
        }
    }
    parent_dept_ids
}

pub async fn get_user_dept_trees(
    db: &Database,
    user_id: i32,
    params: &DeptSearchParams,
) -> Result<Vec<Dept>> {
    let infos = get_depts_by_user_id(db, user_id, params).await?;
    let parent_id = get_tree_start_parent_id::<Info>(&infos);
    Ok(vec_to_tree_into::<Dept, Info>(&parent_id, &infos))
}

pub async fn info(db: &Database, id: i32) -> Result<Info> {
    Ok(db
        .client
        .system_dept()
        .find_first(vec![
            system_dept::id::equals(id),
            system_dept::deleted_at::equals(None),
        ])
        .exec()
        .await?
        .ok_or(ServiceError::DataNotFound)?
        .into())
}

async fn get_dept_tree(db: &Database, params: &DeptSearchParams) -> Result<Vec<Dept>> {
    let infos = get_depts(db, params).await?;
    let parent_id = get_tree_start_parent_id::<Info>(&infos);
    Ok(vec_to_tree_into::<Dept, Info>(&parent_id, &infos))
}

async fn get_depts(db: &Database, params: &DeptSearchParams) -> Result<Vec<Info>> {
    Ok(db
        .client
        .system_dept()
        .find_many(params.to_params())
        .order_by(system_dept::id::order(SortOrder::Asc))
        .exec()
        .await?
        .into_iter()
        .map(|x| x.into())
        .collect::<Vec<Info>>())
}

async fn get_depts_by_user_id(
    db: &Database,
    user_id: i32,
    params: &DeptSearchParams,
) -> Result<Vec<Info>> {
    let user_permission = sys_user::get_current_user_info(db, user_id).await?;

    Ok(
        match (user_permission.get_role(), user_permission.get_dept()) {
            (Some(role), Some(dept)) => {
                let depts = get_role_dept(db, role, params).await?;
                if !depts.is_empty() {
                    depts
                } else {
                    get_children_dept(get_depts(db, params).await?, dept.id)
                }
            }
            (None, Some(dept)) => get_children_dept(get_depts(db, params).await?, dept.id),
            (Some(role), None) => get_role_dept(db, role, params).await?,
            _ => vec![],
        },
    )
}

async fn get_role_dept(
    db: &Database,
    role: sys_role::Info,
    params: &DeptSearchParams,
) -> Result<Vec<Info>> {
    if role.get_sign().as_str().eq(&db.config.admin_role_sign) {
        return get_depts(db, params).await;
    }
    Ok(vec![])
}

fn get_children_dept(depts: Vec<Info>, dept_id: i32) -> Vec<Info> {
    let mut new_depts = vec![];
    for dept in depts.clone() {
        if dept.parent_id.eq(&dept_id) {
            new_depts.push(dept.clone());
            let children = get_children_dept(depts.clone(), dept.id);
            new_depts.extend(children);
        }
    }
    new_depts
}

#[derive(Debug, Serialize)]
pub struct Dept {
    #[serde(flatten)]
    info: Info,
    children: Vec<Dept>,
}

impl From<Info> for Dept {
    fn from(value: Info) -> Self {
        Self {
            info: value,
            children: vec![],
        }
    }
}

impl Tree<Dept> for Dept {
    fn set_child(&mut self, data: Vec<Dept>) {
        self.children = data;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    id: i32,
    parent_id: i32,
    name: String,
    person_name: String,
    person_phone: String,
    person_email: String,
    describe: String,
    status: bool,
    sort: i32,
    created_at: String,
}

impl TreeInfo for Info {
    fn get_parent_id(&self) -> i32 {
        self.parent_id
    }

    fn get_id(&self) -> i32 {
        self.id
    }
}
impl From<system_dept::Data> for Info {
    fn from(value: system_dept::Data) -> Self {
        Self {
            id: value.id,
            parent_id: value.parent_id,
            name: value.name,
            person_name: value.person_name,
            person_phone: value.person_phone,
            person_email: value.person_email,
            describe: value.describe,
            status: value.status,
            sort: value.sort,
            created_at: to_local_string(value.created_at),
        }
    }
}

#[derive(Debug)]
pub struct DeptSearchParams {
    keyword: Option<String>,
    status: Option<bool>,
}
impl DeptSearchParams {
    fn to_params(&self) -> Vec<system_dept::WhereParam> {
        let mut params = vec![system_dept::deleted_at::equals(None)];
        if let Some(keyword) = &self.keyword {
            params.push(or!(
                system_dept::name::contains(keyword.to_string()),
                system_dept::person_name::contains(keyword.to_string()),
                system_dept::person_email::contains(keyword.to_string()),
                system_dept::person_phone::contains(keyword.to_string()),
                system_dept::describe::contains(keyword.to_string())
            ));
        }
        if let Some(status) = self.status {
            params.push(system_dept::status::equals(status));
        }
        params
    }
    pub fn new(keyword: Option<String>, status: Option<bool>) -> Self {
        Self { keyword, status }
    }
}

system_dept::partial_unchecked!(DeptCreateParams {
    parent_id
    person_name
    person_phone
    person_email
    describe
    status
    sort
});

system_dept::partial_unchecked!(DeptUpdateParams {
    parent_id
    name
    person_name
    person_phone
    person_email
    describe
    status
    sort
});
