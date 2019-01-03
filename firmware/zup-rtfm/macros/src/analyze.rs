use std::{
    cmp,
    collections::{BTreeMap, HashMap, HashSet},
    ops,
};

use syn::{Ident, Type};

use crate::syntax::App;

pub type Ownerships = HashMap<Ident, Ownership>;

pub struct Analysis {
    pub dispatchers: Vec<BTreeMap</* priority: */ u8, Dispatcher>>,
    pub resources_assert_send: HashSet<Box<Type>>,
    pub tasks_assert_send: HashSet<Ident>,
    /// Types of RO resources that need to be Sync
    pub assert_sync: HashSet<Box<Type>>,
    // Resource ownership
    pub ownerships: Ownerships,
    pub tasks: HashMap<Ident, Task>,
}

#[derive(Default)]
pub struct Task {
    pub local: FreeQueue,
    pub shared: FreeQueue,
}

#[derive(Default)]
pub struct FreeQueue {
    pub capacity: u8,
    pub ceiling: u8,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Ownership {
    // NOTE priorities and ceilings are "logical" (0 = lowest priority, 255 = highest priority)
    Owned { priority: u8 },
    CoOwned { priority: u8 },
    Shared { ceiling: u8 },
}

impl Ownership {
    pub fn needs_lock(&self, priority: u8) -> bool {
        match *self {
            Ownership::Owned { .. } | Ownership::CoOwned { .. } => false,
            Ownership::Shared { ceiling } => {
                debug_assert!(ceiling >= priority);

                priority < ceiling
            }
        }
    }

    pub fn is_owned(&self) -> bool {
        match *self {
            Ownership::Owned { .. } => true,
            _ => false,
        }
    }
}

pub struct Dispatcher {
    /// Number in the range `0..=15`
    pub sgi: u8,
    /// Tasks dispatched at this priority level
    pub tasks: Vec<(Ident, /* cross: */ bool)>,
    // Queue capacity
    pub capacity: u8,
    // Ready queue ceiling
    pub ceiling: u8,
    // is the queue in shared memory?
    pub shared: bool,
}

// TODO remove?
// #[derive(Clone, Copy, PartialEq)]
// pub enum Source {
//     /// All messages are local
//     Local,
//     /// All messages come from another core
//     External,
//     Both,
// }

// impl ops::AddAssign<Source> for Source {
//     fn add_assign(&mut self, other: Source) {
//         match *self {
//             Source::Local if other != Source::Local => *self = Source::Both,
//             Source::External if other != Source::External => *self = Source::Both,
//             _ => {}
//         }
//     }
// }

pub fn app(app: &App) -> Analysis {
    // Ceiling analysis of R/W resource and Sync analysis of RO resources
    // (Resource shared by tasks that run at different priorities need to be `Sync`)
    let mut ownerships = Ownerships::new();
    let mut assert_sync = HashSet::new();

    for (priority, res) in app.resource_accesses() {
        // `#[shared]` statics are not resources
        if app.resources[res].core.is_none() {
            continue;
        }

        if let Some(ownership) = ownerships.get_mut(res) {
            match *ownership {
                Ownership::Owned { priority: ceiling }
                | Ownership::CoOwned { priority: ceiling }
                | Ownership::Shared { ceiling }
                    if priority != ceiling =>
                {
                    *ownership = Ownership::Shared {
                        ceiling: cmp::max(ceiling, priority),
                    };

                    let res = &app.resources[res];
                    if res.mutability.is_none() {
                        assert_sync.insert(res.ty.clone());
                    }
                }
                Ownership::Owned { priority: ceiling } if ceiling == priority => {
                    *ownership = Ownership::CoOwned { priority };
                }
                _ => {}
            }

            continue;
        }

        ownerships.insert(res.clone(), Ownership::Owned { priority });
    }

    // Compute sizes of free queues
    // We assume at most one message per `spawn` / `schedule`
    let mut tasks: HashMap<_, _> = app
        .tasks
        .keys()
        .map(|task| (task.clone(), Task::default()))
        .collect();
    for spawn in app.spawn_calls() {
        let task = tasks.get_mut(spawn.task).expect("unreachable");

        if spawn.cross {
            task.shared.capacity += 1;
        } else {
            task.local.capacity += 1;
        }
    }

    // TODO need some way to only override the local queue or the shared queue
    // Override computed capacities if user specified a capacity in `#[task]`
    for (name, task) in &app.tasks {
        if let Some(cap) = task.args.capacity {
            let task = tasks.get_mut(name).expect("unreachable");

            task.local.capacity = cap;
            task.shared.capacity = cap;
        }
    }

    // Compute dispatchers capacities
    // Determine which tasks are dispatched by which dispatcher
    let mut dispatchers: Vec<_> = (0..app.cores)
        .map(|core| {
            let mut dispatchers = BTreeMap::new();

            let mut sorted_tasks = app
                .tasks
                .iter()
                .filter(|(_, task)| task.core == core)
                .collect::<Vec<_>>();

            sorted_tasks.sort_by(|l, r| l.1.args.priority.cmp(&r.1.args.priority));

            let mut sgi = 0;
            for (name, task) in sorted_tasks {
                let dispatcher =
                    dispatchers
                        .entry(task.args.priority)
                        .or_insert_with(|| Dispatcher {
                            sgi: {
                                let old = sgi;
                                sgi += 1;
                                old
                            },
                            capacity: 0,
                            ceiling: 0,
                            tasks: vec![],
                            shared: false,
                        });

                let task = &tasks[name];
                dispatcher.capacity += task.local.capacity + task.shared.capacity;

                if task.local.capacity != 0 {
                    dispatcher.tasks.push((name.clone(), false));
                }

                if task.shared.capacity != 0 {
                    dispatcher.shared = true;
                    dispatcher.tasks.push((name.clone(), true));
                }
            }

            dispatchers
        })
        .collect();

    // All messages sent from `init` need to be `Send`
    let mut tasks_assert_send = HashSet::new();
    for task in app.mains.iter().flat_map(|main| &main.init.args.spawn) {
        tasks_assert_send.insert(task.clone());
    }

    // All late resources need to be `Send`, unless they are owned by `idle`
    let mut resources_assert_send = HashSet::new();
    for (name, res) in &app.resources {
        let owned_by_idle = Ownership::Owned { priority: 0 };
        if res.expr.is_none()
            && ownerships
                .get(name)
                .map(|ship| *ship != owned_by_idle)
                .unwrap_or(false)
        {
            resources_assert_send.insert(res.ty.clone());
        }
    }

    // All resources shared with init need to be `Send`, unless they are owned by `idle`
    // This is equivalent to late initialization (e.g. `static mut LATE: Option<T> = None`)
    for name in app.mains.iter().flat_map(|main| &main.init.args.resources) {
        let owned_by_idle = Ownership::Owned { priority: 0 };
        if ownerships
            .get(name)
            .map(|ship| *ship != owned_by_idle)
            .unwrap_or(false)
        {
            resources_assert_send.insert(app.resources[name].ty.clone());
        }
    }

    // Ceiling analysis of free queues (consumer end point)
    // Ceiling analysis of ready queues (producer end point)
    for spawn in app.spawn_calls() {
        if let Some(prio) = spawn.priority {
            // Users of `spawn` contend for the to-be-spawned task FREE_QUEUE and ..
            let task = tasks.get_mut(&spawn.task).expect("unreachable");
            if spawn.cross {
                task.shared.ceiling = cmp::max(task.shared.ceiling, prio);
            } else {
                task.local.ceiling = cmp::max(task.local.ceiling, prio);
            }

            // .. also for the dispatcher READY_QUEUE
            let task = &app.tasks[spawn.task];
            let dispatcher = dispatchers[usize::from(task.core)]
                .get_mut(&task.args.priority)
                .expect("unreachable");
            dispatcher.ceiling = cmp::max(dispatcher.ceiling, prio);

            // Send is required when sending messages from a task whose priority doesn't match the
            // priority of the receiving task
            if task.args.priority != prio {
                tasks_assert_send.insert(spawn.task.clone());
            }
        } else {
            // spawns from `init` are excluded from the ceiling analysis
        }
    }

    Analysis {
        assert_sync,
        dispatchers,
        ownerships,
        resources_assert_send,
        tasks,
        tasks_assert_send,
    }
}
