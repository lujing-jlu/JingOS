use spin::Mutex;

pub const MAX_TASKS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Sleeping,
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
    pub sleep_until_tick: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct SchedulerSnapshot {
    pub total: usize,
    pub ready: usize,
    pub running: usize,
    pub sleeping: usize,
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
            sleep_until_tick: None,
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
            sleep_until_tick: None,
        });
        Ok(id)
    }

    fn wake_due(&mut self, now_ticks: u64) -> usize {
        let mut woke = 0_usize;

        for slot in self.slots.iter_mut() {
            let Some(task) = slot.as_mut() else {
                continue;
            };

            if task.state != TaskState::Sleeping {
                continue;
            }

            if task
                .sleep_until_tick
                .is_some_and(|until_tick| now_ticks >= until_tick)
            {
                task.state = TaskState::Ready;
                task.sleep_until_tick = None;
                woke = woke.saturating_add(1);
            }
        }

        woke
    }

    fn step(&mut self, now_ticks: u64) -> Option<TaskStepReport> {
        self.wake_due(now_ticks);

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
        task.sleep_until_tick = None;
        self.slots[index] = Some(task);

        let report = TaskStepReport {
            id: task.id,
            kind: task.kind,
            run_count: task.run_count,
        };

        task.state = TaskState::Ready;
        task.sleep_until_tick = None;
        self.slots[index] = Some(task);

        Some(report)
    }

    fn sleep(&mut self, id: u64, sleep_ticks: u64, now_ticks: u64) -> Result<u64, &'static str> {
        if sleep_ticks == 0 {
            return Err("sleep ticks must be > 0");
        }

        self.wake_due(now_ticks);

        for slot in self.slots.iter_mut() {
            let Some(task) = slot.as_mut() else {
                continue;
            };

            if task.id != id {
                continue;
            }

            if task.state == TaskState::Running {
                return Err("task is running");
            }

            let until_tick = now_ticks.saturating_add(sleep_ticks);
            task.state = TaskState::Sleeping;
            task.sleep_until_tick = Some(until_tick);
            return Ok(until_tick);
        }

        Err("task not found")
    }

    fn snapshot(&self) -> SchedulerSnapshot {
        let mut total = 0;
        let mut ready = 0;
        let mut running = 0;
        let mut sleeping = 0;

        for task in self.slots.iter().flatten() {
            total += 1;
            match task.state {
                TaskState::Ready => ready += 1,
                TaskState::Running => running += 1,
                TaskState::Sleeping => sleeping += 1,
            }
        }

        SchedulerSnapshot {
            total,
            ready,
            running,
            sleeping,
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

pub fn step(now_ticks: u64) -> Option<TaskStepReport> {
    let mut table = TASK_TABLE.lock();
    table.step(now_ticks)
}

pub fn sleep(id: u64, sleep_ticks: u64, now_ticks: u64) -> Result<u64, &'static str> {
    let mut table = TASK_TABLE.lock();
    table.sleep(id, sleep_ticks, now_ticks)
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
        TaskState::Sleeping => "sleeping",
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
