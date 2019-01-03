use std::collections::HashSet;

use proc_macro2::Span;
use syn::parse;

use crate::syntax::App;

/// Number of SGIs provided by the hardware
const NSGIS: usize = 16;

pub fn app(app: &App) -> parse::Result<()> {
    // Check that all referenced resources have been declared and that they are accessible from that
    // core
    for (ctxt, res) in
        app.mains
            .iter()
            .zip(0..)
            .flat_map(|(main, core)| {
                main.init
                    .args
                    .resources
                    .iter()
                    .map(move |res| (core, res))
                    .chain(main.idle.iter().flat_map(move |idle| {
                        idle.args.resources.iter().map(move |res| (core, res))
                    }))
            })
            .chain(app.tasks.values().flat_map(|task| {
                let core = task.core;
                task.args.resources.iter().map(move |res| (core, res))
            }))
    {
        let span = res.span();
        if let Some(res) = app.resources.get(res) {
            if let Some(owner) = res.core {
                if ctxt != owner {
                    return Err(parse::Error::new(
                        span,
                        "this resource can NOT be accessed from this context",
                    ));
                }
            } else {
                // `#[shared]` statics can be accessed from any core
            }
        } else {
            return Err(parse::Error::new(
                span,
                "this resource has NOT been declared",
            ));
        }
    }

    for init in app.mains.iter().map(|main| &main.init) {
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

    // Check that all late resources have been initialized in `#[init]`
    for (core, res) in app.resources.iter().filter_map(|(name, res)| {
        if res.expr.is_none() {
            Some((res.core.expect("BUG: `#[shared]` late resource"), name))
        } else {
            None
        }
    }) {
        if app.mains[usize::from(core)]
            .init
            .assigns
            .iter()
            .all(|assign| assign.left != *res)
        {
            return Err(parse::Error::new(
                res.span(),
                "late resources MUST be initialized at the end of `init`",
            ));
        }
    }

    // Check that all referenced tasks have been declared
    for task in app
        .mains
        .iter()
        .flat_map(|main| {
            main.init
                .args
                .spawn
                .iter()
                .chain(main.idle.iter().flat_map(|idle| &idle.args.spawn))
        })
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
                if task.core == core {
                    Some(task.args.priority)
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>()
            .len();

        if ndispatchers > NSGIS {
            return Err(parse::Error::new(
                Span::call_site(),
                &*format!(
                    "It's not possible to have more than {} different priority levels for tasks",
                    NSGIS,
                ),
            ));
        }
    }

    Ok(())
}
