use crate::domain::{ProjectSnapshot, WritePlan};
use crate::error::{AppError, AppResult};
use crate::task::TaskManager;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectToken {
    pub project_id: Uuid,
    pub revision: u64,
}

struct StoredWritePlan {
    project_id: Uuid,
    plan: WritePlan,
}

pub struct AppState {
    project: RwLock<Option<ProjectSnapshot>>,
    project_path: RwLock<Option<PathBuf>>,
    write_plans: RwLock<BTreeMap<Uuid, StoredWritePlan>>,
    pub tasks: TaskManager,
    pub resource_dir: PathBuf,
    dirty: AtomicBool,
    revision: AtomicU64,
}

impl AppState {
    pub fn new(resource_dir: PathBuf) -> Self {
        Self {
            project: RwLock::new(None),
            project_path: RwLock::new(None),
            write_plans: RwLock::new(BTreeMap::new()),
            tasks: TaskManager::default(),
            resource_dir,
            dirty: AtomicBool::new(false),
            revision: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> AppResult<ProjectSnapshot> {
        self.project
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))
    }

    pub fn project_path(&self) -> AppResult<PathBuf> {
        self.project_path
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or_else(|| AppError::project_invalid("当前项目尚未关联项目文件。"))
    }

    pub fn snapshot_with_token(&self) -> AppResult<(ProjectSnapshot, ProjectToken)> {
        let guard = self
            .project
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = guard
            .as_ref()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))?;
        let token = ProjectToken {
            project_id: project.project.id,
            revision: self.revision.load(Ordering::Acquire),
        };
        Ok((project.clone(), token))
    }

    pub fn project_token(&self) -> AppResult<ProjectToken> {
        self.snapshot_with_token().map(|(_, token)| token)
    }

    pub fn replace_project(&self, path: PathBuf, snapshot: ProjectSnapshot, dirty: bool) {
        *self
            .project
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(snapshot);
        *self
            .project_path
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(path);
        self.write_plans
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.dirty.store(dirty, Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Release);
    }

    pub fn mutate_project_if_current<T>(
        &self,
        token: ProjectToken,
        operation: impl FnOnce(&mut ProjectSnapshot) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self
            .project
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = guard
            .as_mut()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))?;
        if project.project.id != token.project_id
            || self.revision.load(Ordering::Acquire) != token.revision
        {
            return Err(stale_project_error());
        }
        let result = operation(project)?;
        drop(guard);
        self.write_plans
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.dirty.store(true, Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Release);
        Ok(result)
    }

    pub fn commit_saved_if_current(
        &self,
        token: ProjectToken,
        path: PathBuf,
        snapshot: ProjectSnapshot,
    ) -> AppResult<()> {
        let mut guard = self
            .project
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = guard
            .as_ref()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))?;
        if project.project.id != token.project_id
            || self.revision.load(Ordering::Acquire) != token.revision
        {
            return Err(stale_project_error());
        }
        *guard = Some(snapshot);
        *self
            .project_path
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(path);
        self.write_plans
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.dirty.store(false, Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Release);
        Ok(())
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub fn insert_write_plan_if_current(
        &self,
        token: ProjectToken,
        plan: WritePlan,
    ) -> AppResult<()> {
        let project_guard = self
            .project
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = project_guard
            .as_ref()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))?;
        if project.project.id != token.project_id
            || self.revision.load(Ordering::Acquire) != token.revision
        {
            return Err(stale_project_error());
        }
        self.write_plans
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                plan.id,
                StoredWritePlan {
                    project_id: token.project_id,
                    plan,
                },
            );
        Ok(())
    }

    pub fn write_plan_with_token(&self, id: Uuid) -> AppResult<(WritePlan, ProjectToken)> {
        let project_guard = self
            .project
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project_id = project_guard
            .as_ref()
            .ok_or_else(|| AppError::project_invalid("当前没有打开的项目。"))?
            .project
            .id;
        let token = ProjectToken {
            project_id,
            revision: self.revision.load(Ordering::Acquire),
        };
        let plans = self
            .write_plans
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        plans
            .get(&id)
            .filter(|stored| stored.project_id == project_id)
            .map(|stored| stored.plan.clone())
            .map(|plan| (plan, token))
            .ok_or_else(|| AppError::invalid("写入计划不存在或已失效。"))
    }
}

fn stale_project_error() -> AppError {
    AppError::new(
        crate::error::ErrorCode::ProjectInvalid,
        "项目在任务执行期间已切换或发生变化，结果未写入当前项目。",
        "请在当前项目中重新执行该操作。",
        true,
    )
}
