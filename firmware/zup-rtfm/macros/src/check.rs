use std::collections::{
    hash_map::{Entry, HashMap},
    HashSet,
};

use proc_macro2::Span;
use syn::parse;

use crate::{syntax::App, NSGIS};

pub fn app(app: &App) -> parse::Result<()> {
    // in single-core context no static should use the `#[global]` attribute
    if app.cores == 1 {
        let main = &app.mains[0];
        for (name, static_) in main
            .init
            .iter()
            .flat_map(|init| &init.statics)
            .chain(main.idle.iter().flat_map(|idle| &idle.statics))
            .chain(app.tasks.values().flat_map(|task| &task.statics))
        {
            if static_.global {
                return Err(parse::Error::new(
                    name.span(),
                    "statics can NOT be marked as `#[global]` in single-core applications",
                ));
            }
        }
    }

    for (name, static_) in app.tasks.values().flat_map(|task| &task.statics) {
        if static_.global {
            return Err(parse::Error::new(
                name.span(),
                "statics within a `#[task]` can NOT be marked as `#[global]`",
            ));
        }
    }

    // Check that all referenced resources have been declared and that `static mut` resources are
    // *not* shared between cores
    let mut mut_resources = HashMap::new();
    for (core, name) in
        app.mains
            .iter()
            .zip(0..)
            .flat_map(move |(main, core)| {
                main.init
                    .iter()
                    .flat_map(move |init| init.args.resources.iter().map(move |res| (core, res)))
                    .chain(main.idle.iter().flat_map(move |idle| {
                        idle.args.resources.iter().map(move |res| (core, res))
                    }))
            })
            .chain(app.interrupts.values().flat_map(|interrupt| {
                let core = interrupt.args.core;
                interrupt.args.resources.iter().map(move |res| (core, res))
            }))
            .chain(app.tasks.values().flat_map(|task| {
                let core = task.args.core;
                task.args.resources.iter().map(move |res| (core, res))
            }))
    {
        let span = name.span();
        if let Some(res) = app.resources.get(name) {
            if res.mutability.is_some() {
                match mut_resources.entry(name) {
                    Entry::Occupied(entry) => {
                        if *entry.get() != core {
                            return Err(parse::Error::new(
                                span,
                                "`static mut` resources can NOT be accessed from different cores",
                            ));
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(core);
                    }
                }
            }
        } else {
            return Err(parse::Error::new(
                span,
                "this resource has NOT been declared",
            ));
        }
    }

    for init in app.mains.iter().filter_map(|main| main.init.as_ref()) {
        // Check that late resources have not been assigned to `init`
        for res in &init.args.resources {
            if app.resources.get(res).unwrap().expr.is_none() {
                return Err(parse::Error::new(
                    res.span(),
                    "late resources can NOT be assigned to `init`",
                ));
            }
        }
    }

    // Check that all late resources are covered by `init::LateResources`
    let mut late_resources = app
        .resources
        .iter()
        .filter_map(|(name, res)| if res.expr.is_none() { Some(name) } else { None })
        .collect::<HashSet<_>>();
    if !late_resources.is_empty() {
        if app.cores == 1 {
            // the only core will initialize all late resources
        } else {
            // this core will initialize the "rest" of late resources
            let mut rest = None;

            let mut initialized = HashMap::new();
            for (core, init) in app.mains.iter().enumerate().filter_map(|(i, main)| {
                if let Some(init) = main.init.as_ref() {
                    if init.returns_late_resources {
                        Some((i, init))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                if !init.args.late.is_empty() {
                    for res in &init.args.late {
                        if !late_resources.contains(&res) {
                            return Err(parse::Error::new(
                                res.span(),
                                "this is not a late resource",
                            ));
                        }

                        if let Some(other) = initialized.get(res) {
                            return Err(parse::Error::new(
                                res.span(),
                                &format!("this resource will be initialized by core {}", other),
                            ));
                        } else {
                            late_resources.remove(res);
                            initialized.insert(res, core);
                        }
                    }
                } else if let Some(rest) = rest {
                    return Err(parse::Error::new(
                        Span::call_site(),
                        &format!(
                            "unclear how initialization of late resources is split between \
                             cores {} and {}",
                            rest, core,
                        ),
                    ));
                } else {
                    rest = Some(core);
                }
            }

            if let Some(res) = late_resources.iter().next() {
                if rest.is_none() {
                    return Err(parse::Error::new(
                        res.span(),
                        "this resource is not being initialized",
                    ));
                }
            }
        }
    }

    // Check that all referenced tasks have been declared
    for task in app
        .mains
        .iter()
        .flat_map(|main| {
            main.init
                .iter()
                .flat_map(|init| &init.args.spawn)
                .chain(main.idle.iter().flat_map(|idle| &idle.args.spawn))
        })
        .chain(
            app.interrupts
                .values()
                .flat_map(|interrupt| &interrupt.args.spawn),
        )
        .chain(app.tasks.values().flat_map(|task| &task.args.spawn))
    {
        if !app.tasks.contains_key(task) {
            return Err(parse::Error::new(
                task.span(),
                "this task has NOT been declared",
            ));
        }
    }

    // Check that there are enough dispatchers to handle all priority levels
    for core in 0..app.cores {
        let ndispatchers = app
            .tasks
            .values()
            .filter_map(|task| {
                if task.args.core == core {
                    Some(task.args.priority)
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>()
            .len();

        let used_sgis = app
            .interrupts
            .keys()
            .filter(|name| {
                let name = name.to_string();

                name.starts_with("SG")
                    && name[2..].parse::<u8>().map(|n| n < NSGIS).unwrap_or(false)
            })
            .count();

        if ndispatchers + usize::from(used_sgis) > usize::from(NSGIS) {
            return Err(parse::Error::new(
                Span::call_site(),
                "Not enough free Software-Generated Interrupts (SGI) to \
                 dispatch all task priorities",
            ));
        }
    }

    Ok(())
}
