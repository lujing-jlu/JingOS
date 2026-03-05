use spin::Mutex;

pub const MAX_TASKS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    KernelMonitor,
    UserDemo,
    FastSyscallSuccess,
    FastSyscallError,
}

#[derive(Debug, Clone, Copy)]
pub struct TaskInfo {
    pub id: u64,
    pub kind: TaskKind,
    pub state: TaskState,
    pub run_count: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SchedulerSnapshot {
    pub total: usize,
    pub ready: usize,
    pub running: usize,
    pub finished: usize,
    pub capacity: usize,
    pub next_id: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TaskStepReport {
    pub id: u64,
    pub kind: TaskKind,
    pub run_count: u64,
}

struct TaskTable {
    slots: [Option<TaskInfo>; MAX_TASKS],
    next_id: u64,
    next_rr_index: usize,
}

impl TaskTable {
    const fn new() -> Self {
        Self {
            slots: [None; MAX_TASKS],
            next_id: 1,
            next_rr_index: 0,
        }
    }

    fn ensure_bootstrap_task(&mut self) {
        if self.slots.iter().flatten().any(|task| task.id == 1) {
            return;
        }

        self.slots[0] = Some(TaskInfo {
            id: 1,
            kind: TaskKind::KernelMonitor,
            state: TaskState::Ready,
            run_count: 0,
        });
        self.next_id = self.next_id.max(2);
    }

    fn spawn(&mut self, kind: TaskKind) -> Result<u64, &'static str> {
        let Some(index) = self.slots.iter().position(Option::is_none) else {
            return Err("task table full");
        };

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.slots[index] = Some(TaskInfo {
            id,
            kind,
            state: TaskState::Ready,
            run_count: 0,
        });
        Ok(id)
    }

    fn step(&mut self) -> Option<TaskStepReport> {
        let mut selected_index = None;
        for offset in 0..MAX_TASKS {
            let index = (self.next_rr_index + offset) % MAX_TASKS;
            if matches!(self.slots[index], Some(task) if task.state == TaskState::Ready) {
                selected_index = Some(index);
                break;
            }
        }

        let index = selected_index?;
        self.next_rr_index = (index + 1) % MAX_TASKS;

        let mut task = self.slots[index]?;
        task.state = TaskState::Running;
        task.run_count = task.run_count.saturating_add(1);
        self.slots[index] = Some(task);

        let report = TaskStepReport {
            id: task.id,
            kind: task.kind,
            run_count: task.run_count,
        };

        task.state = TaskState::Ready;
        self.slots[index] = Some(task);

        Some(report)
    }

    fn snapshot(&self) -> SchedulerSnapshot {
        let mut total = 0;
        let mut ready = 0;
        let mut running = 0;

        for task in self.slots.iter().flatten() {
            total += 1;
            match task.state {
                TaskState::Ready => ready += 1,
                TaskState::Running => running += 1,
            }
        }

        SchedulerSnapshot {
            total,
            ready,
            running,
            finished: 0,
            capacity: MAX_TASKS,
            next_id: self.next_id,
        }
    }

    fn list(&self, output: &mut [Option<TaskInfo>; MAX_TASKS]) -> usize {
        let mut count = 0;
        for task in self.slots.iter().flatten() {
            output[count] = Some(*task);
            count += 1;
        }
        count
    }
}

static TASK_TABLE: Mutex<TaskTable> = Mutex::new(TaskTable::new());

pub fn init() {
    let mut table = TASK_TABLE.lock();
    table.ensure_bootstrap_task();
}

pub fn spawn(kind: TaskKind) -> Result<u64, &'static str> {
    let mut table = TASK_TABLE.lock();
    table.spawn(kind)
}

pub fn step() -> Option<TaskStepReport> {
    let mut table = TASK_TABLE.lock();
    table.step()
}

pub fn snapshot() -> SchedulerSnapshot {
    let table = TASK_TABLE.lock();
    table.snapshot()
}

pub fn list(output: &mut [Option<TaskInfo>; MAX_TASKS]) -> usize {
    let table = TASK_TABLE.lock();
    table.list(output)
}

pub fn task_kind_name(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::KernelMonitor => "kernel_monitor",
        TaskKind::UserDemo => "user_demo",
        TaskKind::FastSyscallSuccess => "fast_syscall_success",
        TaskKind::FastSyscallError => "fast_syscall_error",
    }
}

pub fn task_state_name(state: TaskState) -> &'static str {
    match state {
        TaskState::Ready => "ready",
        TaskState::Running => "running",
    }
}

pub fn parse_task_kind(text: &str) -> Option<TaskKind> {
    match text {
        "monitor" | "kernel" | "kernel_monitor" => Some(TaskKind::KernelMonitor),
        "userdemo" | "user" | "user_demo" => Some(TaskKind::UserDemo),
        "fastsyscall" | "fast" | "fast_ok" | "fastsyscall_ok" | "fast_syscall_success" => {
            Some(TaskKind::FastSyscallSuccess)
        }
        "fastfail" | "fastsyscall_fail" | "fast_syscall_error" | "fast_error" => {
            Some(TaskKind::FastSyscallError)
        }
        _ => None,
    }
}
