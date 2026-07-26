use crate::domain::{TaskKind, TaskRecord, TaskStatus};
use crate::error::AppError;
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct TaskControl {
    record: TaskRecord,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct TaskManager {
    tasks: Mutex<BTreeMap<Uuid, TaskControl>>,
}

impl TaskManager {
    pub fn start(&self, kind: TaskKind, stage: impl Into<String>) -> (Uuid, Arc<AtomicBool>) {
        let id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        let record = TaskRecord {
            id,
            kind,
            status: TaskStatus::Running,
            stage: stage.into(),
            completed: 0,
            total: 0,
            message: "任务已开始。".to_owned(),
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        };
        self.lock_tasks().insert(
            id,
            TaskControl {
                record,
                cancelled: cancelled.clone(),
            },
        );
        (id, cancelled)
    }

    pub fn update(
        &self,
        id: Uuid,
        stage: impl Into<String>,
        completed: u64,
        total: u64,
        message: impl Into<String>,
    ) {
        if let Some(control) = self.lock_tasks().get_mut(&id) {
            control.record.stage = stage.into();
            control.record.completed = completed;
            control.record.total = total;
            control.record.message = message.into();
        }
    }

    pub fn finish(&self, id: Uuid, message: impl Into<String>) {
        if let Some(control) = self.lock_tasks().get_mut(&id) {
            control.record.status = if control.cancelled.load(Ordering::Relaxed) {
                TaskStatus::Cancelled
            } else {
                TaskStatus::Completed
            };
            control.record.message = message.into();
            control.record.finished_at = Some(Utc::now());
        }
    }

    pub fn fail(&self, id: Uuid, error: AppError) {
        if let Some(control) = self.lock_tasks().get_mut(&id) {
            control.record.status = if error.code == crate::error::ErrorCode::TaskCancelled {
                TaskStatus::Cancelled
            } else {
                TaskStatus::Failed
            };
            control.record.message = error.message.clone();
            control.record.error = Some(error);
            control.record.finished_at = Some(Utc::now());
        }
    }

    pub fn cancel(&self, id: Uuid) -> bool {
        let mut tasks = self.lock_tasks();
        let Some(control) = tasks.get_mut(&id) else {
            return false;
        };
        if matches!(
            control.record.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return false;
        }
        control.cancelled.store(true, Ordering::Relaxed);
        control.record.message = "正在安全取消任务…".to_owned();
        true
    }

    pub fn get(&self, id: Uuid) -> Option<TaskRecord> {
        self.lock_tasks()
            .get(&id)
            .map(|control| control.record.clone())
    }

    pub fn list(&self) -> Vec<TaskRecord> {
        self.lock_tasks()
            .values()
            .map(|control| control.record.clone())
            .collect()
    }

    fn lock_tasks(&self) -> std::sync::MutexGuard<'_, BTreeMap<Uuid, TaskControl>> {
        self.tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_idempotently_reported() {
        let manager = TaskManager::default();
        let (id, flag) = manager.start(TaskKind::PhotoScan, "scan");
        assert!(manager.cancel(id));
        assert!(flag.load(Ordering::Relaxed));
        manager.finish(id, "cancelled");
        assert_eq!(
            manager.get(id).expect("record").status,
            TaskStatus::Cancelled
        );
        assert!(!manager.cancel(id));
    }
}
