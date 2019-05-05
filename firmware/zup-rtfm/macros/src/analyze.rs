use core::{cmp, ops};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use syn::{Ident, Type};

use crate::{
    syntax::{App, Idents},
    NSGIS,
};

pub type Ownerships = HashMap<Ident, Ownership>;

pub struct Analysis {
    /// Per core dispatchers
    pub dispatchers: Dispatchers,

    /// Ceilings of free queues
    pub free_queues: FreeQueues,

    pub resources_assert_local_send: HashSet<Box<Type>>,
    pub resources_assert_send: HashSet<Box<Type>>,

    pub tasks_assert_local_send: HashSet<Ident>,
    pub tasks_assert_send: HashSet<Ident>,

    /// Types of RO resources that need to be Sync
    pub assert_sync: HashSet<Box<Type>>,

    // Resource ownership
    pub ownerships: Ownerships,

    /// Location of resources, `None` indicates `#[shared]` memory
    // Resources are usually `#[local]` but they must be `#[shared]` when (a) they are shared
    // between cores (RO resources) or (b) they are cross-initialized
    // Resources not accessed by any task will not be listed in this map
    pub locations: BTreeMap<Ident, Option<u8>>,

    /// Maps a core to the resources it initializes
    pub late_resources: BTreeMap<u8, Idents>,

    // priority -> SG{}
    pub sgis: Vec<Sgis>,

    // `receiver` -> [`sender`]
    pub pre_rendezvous: BTreeMap<u8, BTreeSet<u8>>,

    // `user` -> [`initializer`]
    pub post_rendezvous: BTreeMap<u8, BTreeSet<u8>>,
}

pub type Dispatchers = Vec<BTreeMap</* priority: */ u8, BTreeMap</* sender: */ u8, Route>>>;

pub type FreeQueues = BTreeMap<Ident, BTreeMap</* sender: */ u8, /* ceiling: */ Option<u8>>>;

#[derive(Clone, Default)]
pub struct Sgis {
    next: u8,
    map: BTreeMap</* priority: */ u8, /* sgi: */ u8>,
}

impl Sgis {
    fn insert(&mut self, priority: u8, used_sgis: &BTreeSet<u8>) {
        if !self.map.contains_key(&priority) {
            while used_sgis.contains(&self.next) {
                self.next += 1;

                debug_assert!(self.next < NSGIS);
            }

            self.map.insert(priority, self.next);
            self.next += 1;
        }
    }
}

impl ops::Deref for Sgis {
    type Target = BTreeMap<u8, u8>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
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

#[derive(Clone, Default)]
pub struct Route {
    /// Tasks dispatched through this route
    pub tasks: Idents,

    /// The priority ceiling of this route ready queue
    /// `None` means that no task contends for this ready queue; this can happen when spawn is done
    /// by `init`
    pub ceiling: Option<u8>,
}

impl Route {
    pub fn capacity(&self, app: &App) -> u8 {
        self.tasks
            .iter()
            .map(|name| app.tasks[name].args.capacity)
            .sum()
    }
}

pub fn app(app: &App) -> Analysis {
    // Ceiling analysis of R/W resource and Sync analysis of RO resources
    // (RO resources shared by tasks that run at different priorities need to be `Sync`)
    let mut assert_sync = HashSet::new();
    let mut locations = BTreeMap::<_, Option<u8>>::new();
    let mut ownerships = Ownerships::new();

    for (core, priority, name) in app.resource_accesses() {
        let res = &app.resources[name];
        if let Some(location) = locations.get_mut(name) {
            if location.is_some() && location.as_ref() != Some(&core) {
                // shared between different cores
                *location = None;
                debug_assert!(res.mutability.is_none());
                assert_sync.insert(res.ty.clone());
            }
        } else {
            locations.insert(name.clone(), Some(core));
        }

        if let Some(priority) = priority {
            if let Some(ownership) = ownerships.get_mut(name) {
                match *ownership {
                    Ownership::Owned { priority: ceiling }
                    | Ownership::CoOwned { priority: ceiling }
                    | Ownership::Shared { ceiling }
                        if priority != ceiling =>
                    {
                        *ownership = Ownership::Shared {
                            ceiling: cmp::max(ceiling, priority),
                        };

                        if res.mutability.is_none() {
                            assert_sync.insert(res.ty.clone());
                        }
                    }
                    Ownership::Owned { priority: ceiling } if ceiling == priority => {
                        *ownership = Ownership::CoOwned { priority };
                    }
                    _ => {}
                }
            } else {
                ownerships.insert(name.clone(), Ownership::Owned { priority });
            }
        }
    }

    // Determine which core initializes which resource
    let mut late_resources: BTreeMap<_, Idents> = BTreeMap::new();
    let mut resources = app
        .resources
        .iter()
        .filter_map(|(name, res)| {
            if res.expr.is_none() {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect::<Idents>();
    if !resources.is_empty() {
        let mut rest = None;
        for (core, init) in app.mains.iter().zip(0..).filter_map(|(main, core)| {
            main.init.as_ref().and_then(|init| {
                if init.returns_late_resources {
                    Some((core, init))
                } else {
                    None
                }
            })
        }) {
            if init.args.late.is_empty() {
                rest = Some(core);
            } else {
                let late_resources = late_resources.entry(core).or_default();

                for name in &init.args.late {
                    late_resources.insert(name.clone());
                    resources.remove(name);

                    if let Some(location) = locations.get_mut(name) {
                        if location.is_some() && location.as_ref() != Some(&core) {
                            // shared between different cores
                            *location = None;
                        }
                    } else {
                        locations.insert(name.clone(), Some(core));
                    }
                }
            }
        }

        if let Some(rest) = rest {
            for name in &resources {
                if let Some(location) = locations.get_mut(name) {
                    if location.is_some() && location.as_ref() != Some(&rest) {
                        // shared between different cores
                        *location = None;
                    }
                } else {
                    locations.insert(name.clone(), Some(rest));
                }
            }

            late_resources.insert(rest, resources);
        }
    }

    // Check for cross-initialization
    let mut post_rendezvous: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for (user, _, resource) in app.resource_accesses() {
        for (&initializer, resources) in &late_resources {
            if initializer == user {
                continue;
            }

            if resources.contains(resource) {
                post_rendezvous.entry(user).or_default().insert(initializer);
            }
        }
    }

    // All messages sent from `init` need to be `Send` or `LocalSend`
    let mut tasks_assert_send = HashSet::new();
    let mut tasks_assert_local_send = HashSet::new();
    for (sender, task) in app.mains.iter().zip(0..).flat_map(|(main, sender)| {
        main.init
            .iter()
            .flat_map(move |init| init.args.spawn.iter().map(move |task| (sender, task)))
    }) {
        let receiver = app.tasks[task].args.core;

        if sender == receiver {
            tasks_assert_local_send.insert(task.clone());
        } else {
            tasks_assert_send.insert(task.clone());
        }
    }

    let mut resources_assert_local_send = HashSet::new();
    let mut resources_assert_send = HashSet::new();

    // All late resources need to be `Send` or `LocalSend`, except maybe for resources owned by
    // `idle`
    for (name, ty) in app.resources.iter().filter_map(|(name, res)| {
        if res.expr.is_none() {
            Some((name, &res.ty))
        } else {
            None
        }
    }) {
        if locations[name].is_none() {
            // cross-initialized
            resources_assert_send.insert(ty.clone());
        } else {
            let owned_by_idle = Ownership::Owned { priority: 0 };
            if ownerships.get(name).expect("UNREACHABLE") != &owned_by_idle {
                resources_assert_local_send.insert(ty.clone());
            }
        }
    }

    // All resources shared with `init` (ownership != None) need to be `LocalSend`
    for name in app.mains.iter().flat_map(|main| {
        main.init.iter().flat_map(|init| {
            init.args
                .resources
                .iter()
                .filter(|res| ownerships.get(res).is_some())
        })
    }) {
        let owned_by_idle = Ownership::Owned { priority: 0 };
        if ownerships.get(name).expect("UNREACHABLE") != &owned_by_idle {
            resources_assert_local_send.insert(app.resources[name].ty.clone());
        }
    }

    // Assign SGIs to priority levels
    let used_sgis = app
        .interrupts
        .keys()
        .filter_map(|name| {
            let name = name.to_string();

            if name.starts_with("SG") {
                name[2..]
                    .parse::<u8>()
                    .ok()
                    .and_then(|i| if i < NSGIS { Some(i) } else { None })
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let mut sgis: Vec<Sgis> = vec![Sgis::default(); usize::from(app.cores)];
    for task in app.tasks.values() {
        let core = task.args.core;

        sgis[usize::from(core)].insert(task.args.priority, &used_sgis);
    }

    // Ceiling analysis of free queues (consumer end point)
    // Ceiling analysis of ready queues (producer end point)
    let mut pre_rendezvous: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    let mut free_queues = FreeQueues::new();
    let mut dispatchers: Dispatchers = vec![BTreeMap::new(); usize::from(app.cores)];
    for spawn in app.spawn_calls() {
        let task = &app.tasks[spawn.task];
        let spawnee_core = task.args.core;
        let spawnee_priority = task.args.priority;

        if spawn.core != spawnee_core {
            pre_rendezvous
                .entry(spawnee_core)
                .or_default()
                .insert(spawn.core);

            // messages that cross the core boundary need to be `Send`
            tasks_assert_send.insert(spawn.task.clone());
        }

        let dispatcher = dispatchers[usize::from(spawnee_core)]
            .entry(spawnee_priority)
            .or_default();

        let route = dispatcher.entry(spawn.core).or_default();
        route.tasks.insert(spawn.task.clone());

        let fq_ceiling = free_queues
            .entry(spawn.task.clone())
            .or_default()
            .entry(spawn.core)
            .or_default();

        if let Some(priority) = spawn.priority {
            // Spawner task contends for the ready queue
            match route.ceiling {
                None => route.ceiling = Some(priority),
                Some(ceiling) => route.ceiling = Some(cmp::max(priority, ceiling)),
            }

            // Spawner task contends for the free queue
            match fq_ceiling {
                None => *fq_ceiling = Some(priority),
                Some(ceiling) => *fq_ceiling = Some(cmp::max(*ceiling, priority)),
            }

            // LocalSend is required when sending messages from a task whose priority doesn't match
            // the priority of the receiving task
            if spawn.core == spawnee_core && spawnee_priority != priority {
                tasks_assert_local_send.insert(spawn.task.clone());
            }
        } else {
            // spawns from `init` are excluded from the ceiling analysis
        }
    }

    Analysis {
        assert_sync,
        dispatchers,
        free_queues,
        late_resources,
        locations,
        ownerships,
        pre_rendezvous,
        post_rendezvous,
        resources_assert_local_send,
        resources_assert_send,
        sgis,
        tasks_assert_local_send,
        tasks_assert_send,
    }
}
