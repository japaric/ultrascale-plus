use std::{
    collections::{BTreeMap, HashMap, HashSet},
    iter, u8,
};

use proc_macro2::Span;
use syn::{
    braced, bracketed, parenthesized,
    parse::{self, Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Brace,
    ArgCaptured, AttrStyle, Attribute, Expr, FnArg, Ident, IntSuffix, Item, ItemFn, ItemStatic,
    LitInt, LitStr, PathArguments, ReturnType, Stmt, Token, Type, TypeTuple, Visibility,
};

use crate::PRIORITY_BITS;

const MIN_TASK_PRIORITY: u8 = 2;

pub struct AppArgs {
    pub cores: u8,
}

impl Parse for AppArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let mut cores = None;
        loop {
            if input.is_empty() {
                break;
            }

            // #ident = ..
            let ident: Ident = input.parse()?;
            let _eq_token: Token![=] = input.parse()?;

            let ident_s = ident.to_string();
            match &*ident_s {
                "cores" => {
                    if cores.is_some() {
                        return Err(parse::Error::new(
                            ident.span(),
                            "argument appears more than once",
                        ));
                    }

                    let lit: LitInt = input.parse()?;
                    if lit.suffix() != IntSuffix::None {
                        return Err(parse::Error::new(
                            lit.span(),
                            "this integer must be unsuffixed",
                        ));
                    }

                    let val = lit.value();
                    if val < 1 || val > 8 {
                        return Err(parse::Error::new(
                            lit.span(),
                            "number of cores must be in the range 1..=8",
                        ));
                    }

                    cores = Some(val as u8);
                }
                _ => {
                    return Err(parse::Error::new(
                        ident.span(),
                        "expected `cores`; other keys are not accepted",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }

            // ,
            let _: Token![,] = input.parse()?;
        }

        Ok(AppArgs {
            cores: cores.unwrap_or(1),
        })
    }
}

pub struct Input {
    _const_token: Token![const],
    _ident: Ident,
    _colon_token: Token![:],
    _ty: TypeTuple,
    _eq_token: Token![=],
    _brace_token: Brace,
    pub items: Vec<Item>,
    _semi_token: Token![;],
}

impl Parse for Input {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        fn parse_items(input: ParseStream) -> parse::Result<Vec<Item>> {
            let mut items = vec![];

            while !input.is_empty() {
                items.push(input.parse()?);
            }

            Ok(items)
        }

        let content;
        Ok(Input {
            _const_token: input.parse()?,
            _ident: input.parse()?,
            _colon_token: input.parse()?,
            _ty: input.parse()?,
            _eq_token: input.parse()?,
            _brace_token: braced!(content in input),
            items: content.call(parse_items)?,
            _semi_token: input.parse()?,
        })
    }
}

pub struct App {
    pub cores: u8,
    pub mains: Vec<Main>,
    pub resources: Resources,
    pub tasks: Tasks,
}

pub struct Main {
    pub init: Init,
    pub idle: Option<Idle>,
}

pub struct Spawn<'a> {
    // cross-core spawn?
    pub cross: bool,
    // `None` means called from `init`
    pub priority: Option<u8>,
    pub task: &'a Ident,
}

impl App {
    pub fn parse(items: Vec<Item>, args: AppArgs) -> parse::Result<Self> {
        let cores = args.cores;
        let mut idle = (0..cores).map(|_| None).collect::<Vec<_>>();
        let mut init = (0..cores).map(|_| None).collect::<Vec<_>>();
        let mut resources = HashMap::new();
        let mut tasks = HashMap::new();

        for item in items {
            let span = item.span();
            match item {
                Item::Fn(mut item) => {
                    if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "idle")) {
                        let args = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        let core = if cores == 1 {
                            0
                        } else {
                            extract_cfg_core(&mut item.attrs, cores)?.ok_or_else(|| {
                                parse::Error::new(
                                    span,
                                    "this item must be assigned to a single core \
                                     (e.g. `#[cfg(core = \"0\")]`)",
                                )
                            })?
                        };

                        if idle[usize::from(core)].is_some() {
                            return Err(parse::Error::new(
                                span,
                                if cores == 1 {
                                    "`#[idle]` function must appear at most once"
                                } else {
                                    "an `#[idle]` function has already been assigned to this core"
                                },
                            ));
                        }

                        idle[usize::from(core)] = Some(Idle::check(args, item)?);
                    } else if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "init")) {
                        let args = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        let core = if cores == 1 {
                            0
                        } else {
                            extract_cfg_core(&mut item.attrs, cores)?.ok_or_else(|| {
                                parse::Error::new(
                                    span,
                                    "this item must be assigned to a single core \
                                     (e.g. `#[cfg(core = \"0\")]`)",
                                )
                            })?
                        };

                        if init[usize::from(core)].is_some() {
                            return Err(parse::Error::new(
                                span,
                                if cores == 1 {
                                    "`#[init]` function must appear exactly once"
                                } else {
                                    "an `#[init]` function has already been assigned to this core"
                                },
                            ));
                        }

                        init[usize::from(core)] = Some(Init::check(args, item)?);
                    } else if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "task")) {
                        if tasks.contains_key(&item.ident) {
                            return Err(parse::Error::new(
                                item.ident.span(),
                                "this task is defined multiple times",
                            ));
                        }

                        let args = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        tasks.insert(item.ident.clone(), Task::check(args, item, cores)?);
                    } else {
                        return Err(parse::Error::new(
                            span,
                            "this item must live outside the `#[app]` module",
                        ));
                    }
                }
                Item::Static(item) => {
                    if resources.contains_key(&item.ident) {
                        return Err(parse::Error::new(
                            item.ident.span(),
                            "this resource is listed twice",
                        ));
                    }

                    resources.insert(item.ident.clone(), Resource::check(item, cores)?);
                }
                _ => {
                    return Err(parse::Error::new(
                        span,
                        "this item must live outside the `#[app]` module",
                    ));
                }
            }
        }

        let mut mains = Vec::with_capacity(usize::from(cores));
        for ((core, init), idle) in init.into_iter().enumerate().zip(idle) {
            if let Some(init) = init {
                mains.push(Main { init, idle });
            } else {
                return Err(parse::Error::new(
                    Span::call_site(),
                    &format!("core {} is missing the `#[init]`-alization function", core),
                ));
            }
        }

        Ok(App {
            cores,
            mains,
            resources,
            tasks,
        })
    }

    /// Returns an iterator over all resource accesses.
    ///
    /// Each resource access include the priority it's accessed at (`u8`) and the name of the
    /// resource (`Ident`). A resource may appear more than once in this iterator
    pub fn resource_accesses(&self) -> impl Iterator<Item = (u8, &Ident)> {
        self.mains
            .iter()
            .flat_map(|main| {
                main.idle
                    .iter()
                    .flat_map(|idle| idle.args.resources.iter().map(|res| (0, res)))
            })
            .chain(self.tasks.values().flat_map(|task| {
                task.args
                    .resources
                    .iter()
                    .map(move |res| (task.args.priority, res))
            }))
    }

    /// Returns an iterator over all `spawn` calls
    ///
    /// Each spawn call includes the priority of the task from which it's issued and the name of the
    /// task that's spawned. A task may appear more that once in this iterator.
    ///
    /// A priority of `None` means that this being called from `init`
    pub fn spawn_calls(&self) -> impl Iterator<Item = Spawn> {
        self.mains
            .iter()
            .zip(0..)
            .flat_map(move |(main, caller)| {
                main.init
                    .args
                    .spawn
                    .iter()
                    .map(move |task| Spawn {
                        cross: caller != self.tasks[task].core,
                        priority: None,
                        task,
                    })
                    .chain(main.idle.iter().flat_map(move |idle| {
                        idle.args.spawn.iter().map(move |task| Spawn {
                            cross: caller != self.tasks[task].core,
                            priority: Some(0),
                            task,
                        })
                    }))
            })
            .chain(self.tasks.values().flat_map(move |task| {
                task.args.spawn.iter().map(move |callee| Spawn {
                    cross: task.core != self.tasks[callee].core,
                    priority: Some(task.args.priority),
                    task: callee,
                })
            }))
    }

    pub fn spawn_callers(&self) -> impl Iterator<Item = (/* core: */ u8, Ident, &Idents)> {
        self.mains
            .iter()
            .zip(0..)
            .flat_map(move |(main, core)| {
                iter::once((
                    core,
                    Ident::new("init", Span::call_site()),
                    &main.init.args.spawn,
                ))
                .chain(main.idle.iter().map(move |idle| {
                    (
                        core,
                        Ident::new("idle", Span::call_site()),
                        &idle.args.spawn,
                    )
                }))
            })
            .chain(
                self.tasks
                    .iter()
                    .map(|(name, task)| (task.core, name.clone(), &task.args.spawn)),
            )
    }
}

pub type Idents = HashSet<Ident>;
pub type Resources = HashMap<Ident, Resource>;
pub type Statics = Vec<ItemStatic>;
pub type Tasks = HashMap<Ident, Task>;

pub struct Static {
    pub attrs: Vec<Attribute>,
    pub ty: Box<Type>,
    pub expr: Box<Expr>,
}

impl Static {
    fn parse(items: Vec<ItemStatic>) -> parse::Result<HashMap<Ident, Static>> {
        let mut statics = HashMap::new();

        for item in items {
            if statics.contains_key(&item.ident) {
                return Err(parse::Error::new(
                    item.ident.span(),
                    "this `static` is listed twice",
                ));
            }

            statics.insert(
                item.ident,
                Static {
                    attrs: item.attrs,
                    ty: item.ty,
                    expr: item.expr,
                },
            );
        }

        Ok(statics)
    }
}

pub struct Assign {
    pub left: Ident,
    pub right: Box<Expr>,
}

pub struct InitArgs {
    pub resources: Idents,
    pub spawn: Idents,
}

impl Default for InitArgs {
    fn default() -> Self {
        InitArgs {
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

impl Parse for InitArgs {
    fn parse(input: ParseStream) -> parse::Result<InitArgs> {
        if input.is_empty() {
            return Ok(InitArgs::default());
        }

        let mut resources = None;
        let mut spawn = None;

        let content;
        parenthesized!(content in input);
        loop {
            if content.is_empty() {
                break;
            }

            // #ident = ..
            let ident: Ident = content.parse()?;
            let _: Token![=] = content.parse()?;

            let ident_s = ident.to_string();
            match &*ident_s {
                "resources" | "spawn" => {} // OK
                _ => {
                    return Err(parse::Error::new(
                        ident.span(),
                        "expected: resources or spawn",
                    ));
                }
            }

            // .. [#(#idents)*]
            let inner;
            bracketed!(inner in content);
            let mut idents = Idents::new();
            for ident in inner.call(Punctuated::<_, Token![,]>::parse_terminated)? {
                if idents.contains(&ident) {
                    return Err(parse::Error::new(
                        ident.span(),
                        "element appears more than once in list",
                    ));
                }

                idents.insert(ident);
            }

            let ident_s = ident.to_string();
            match &*ident_s {
                "resources" => {
                    if resources.is_some() {
                        return Err(parse::Error::new(
                            ident.span(),
                            "argument appears more than once",
                        ));
                    }

                    resources = Some(idents);
                }
                "spawn" => {
                    if spawn.is_some() {
                        return Err(parse::Error::new(
                            ident.span(),
                            "argument appears more than once",
                        ));
                    }

                    spawn = Some(idents);
                }
                _ => unreachable!(),
            }

            if content.is_empty() {
                break;
            }

            // ,
            let _: Token![,] = content.parse()?;
        }

        Ok(InitArgs {
            resources: resources.unwrap_or(Idents::new()),
            spawn: spawn.unwrap_or(Idents::new()),
        })
    }
}

pub struct Init {
    pub args: InitArgs,
    pub attrs: Vec<Attribute>,
    pub unsafety: Option<Token![unsafe]>,
    pub statics: HashMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
    pub assigns: Vec<Assign>,
}

impl Init {
    fn check(args: InitArgs, item: ItemFn) -> parse::Result<Self> {
        let valid_signature = item.vis == Visibility::Inherited
            && item.constness.is_none()
            && item.asyncness.is_none()
            && item.abi.is_none()
            && item.decl.generics.params.is_empty()
            && item.decl.generics.where_clause.is_none()
            && item.decl.inputs.is_empty()
            && item.decl.variadic.is_none()
            && is_unit(&item.decl.output);

        let span = item.span();

        if !valid_signature {
            return Err(parse::Error::new(
                span,
                "`init` must have type signature `[unsafe] fn()`",
            ));
        }

        let (statics, stmts) = extract_statics(item.block.stmts);
        let (stmts, assigns) = extract_assignments(stmts);

        Ok(Init {
            args,
            attrs: item.attrs,
            unsafety: item.unsafety,
            statics: Static::parse(statics)?,
            stmts,
            assigns,
        })
    }
}

pub type IdleArgs = InitArgs;

pub struct Idle {
    pub args: IdleArgs,
    pub attrs: Vec<Attribute>,
    pub unsafety: Option<Token![unsafe]>,
    pub statics: HashMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Idle {
    fn check(args: IdleArgs, item: ItemFn) -> parse::Result<Self> {
        let valid_signature = item.vis == Visibility::Inherited
            && item.constness.is_none()
            && item.asyncness.is_none()
            && item.abi.is_none()
            && item.decl.generics.params.is_empty()
            && item.decl.generics.where_clause.is_none()
            && item.decl.inputs.is_empty()
            && item.decl.variadic.is_none()
            && is_bottom(&item.decl.output);

        let span = item.span();

        if !valid_signature {
            return Err(parse::Error::new(
                span,
                "`idle` must have type signature `[unsafe] fn() -> !`",
            ));
        }

        let (statics, stmts) = extract_statics(item.block.stmts);

        Ok(Idle {
            args,
            attrs: item.attrs,
            unsafety: item.unsafety,
            statics: Static::parse(statics)?,
            stmts,
        })
    }
}

pub struct Resource {
    pub core: Option<u8>,
    pub attrs: Vec<Attribute>,
    pub mutability: Option<Token![mut]>,
    pub ty: Box<Type>,
    pub expr: Option<Box<Expr>>,
}

impl Resource {
    fn check(mut item: ItemStatic, cores: u8) -> parse::Result<Resource> {
        let span = item.span();
        let core = extract_cfg_core(&mut item.attrs, cores)?.or_else(|| {
            if cores == 1 {
                Some(0)
            } else {
                None
            }
        });

        if item.vis != Visibility::Inherited {
            return Err(parse::Error::new(
                span,
                "resources must have inherited / private visibility",
            ));
        }

        let uninitialized = match *item.expr {
            Expr::Tuple(ref tuple) => tuple.elems.is_empty(),
            _ => false,
        };

        if core.is_none() && uninitialized {
            return Err(parse::Error::new(
                span,
                "Late resources cannot be shared between cores",
            ));
        }

        Ok(Resource {
            core,
            attrs: item.attrs,
            mutability: item.mutability,
            ty: item.ty,
            expr: if uninitialized { None } else { Some(item.expr) },
        })
    }
}

pub struct TaskArgs {
    pub capacity: Option<u8>,
    pub priority: u8,
    pub resources: Idents,
    pub spawn: Idents,
}

impl Default for TaskArgs {
    fn default() -> Self {
        TaskArgs {
            capacity: None,
            priority: MIN_TASK_PRIORITY,
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

impl Parse for TaskArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        parse_args(input, true)
    }
}

// Parser shared by TaskArgs and ExceptionArgs / InterruptArgs
fn parse_args(input: ParseStream, accept_capacity: bool) -> parse::Result<TaskArgs> {
    if input.is_empty() {
        return Ok(TaskArgs::default());
    }

    let mut capacity = None;
    let mut priority = None;
    let mut resources = None;
    let mut spawn = None;

    let content;
    parenthesized!(content in input);
    loop {
        if content.is_empty() {
            break;
        }

        // #ident = ..
        let ident: Ident = content.parse()?;
        let _: Token![=] = content.parse()?;

        let ident_s = ident.to_string();
        match &*ident_s {
            "capacity" if accept_capacity => {
                // #lit
                let lit: LitInt = content.parse()?;

                if lit.suffix() != IntSuffix::None {
                    return Err(parse::Error::new(
                        lit.span(),
                        "this literal must be unsuffixed",
                    ));
                }

                let value = lit.value();
                if value > u64::from(u8::MAX) || value == 0 {
                    return Err(parse::Error::new(
                        lit.span(),
                        "this literal must be in the range 1...255",
                    ));
                }

                capacity = Some(value as u8);
            }
            "priority" => {
                // #lit
                let lit: LitInt = content.parse()?;

                if lit.suffix() != IntSuffix::None {
                    return Err(parse::Error::new(
                        lit.span(),
                        "this literal must be unsuffixed",
                    ));
                }

                let value = lit.value();
                let max_prio = (1u8 << PRIORITY_BITS) - 1;
                if value > u64::from(max_prio) || value < 2 {
                    return Err(parse::Error::new(
                        lit.span(),
                        format!("this literal must be in the range 2..={}", max_prio),
                    ));
                }

                priority = Some(value as u8);
            }
            "resources" | "spawn" => {
                // .. [#(#idents)*]
                let inner;
                bracketed!(inner in content);
                let mut idents = Idents::new();
                for ident in inner.call(Punctuated::<_, Token![,]>::parse_terminated)? {
                    if idents.contains(&ident) {
                        return Err(parse::Error::new(
                            ident.span(),
                            "element appears more than once in list",
                        ));
                    }

                    idents.insert(ident);
                }

                match &*ident_s {
                    "resources" => {
                        if resources.is_some() {
                            return Err(parse::Error::new(
                                ident.span(),
                                "argument appears more than once",
                            ));
                        }

                        resources = Some(idents);
                    }
                    "spawn" => {
                        if spawn.is_some() {
                            return Err(parse::Error::new(
                                ident.span(),
                                "argument appears more than once",
                            ));
                        }

                        spawn = Some(idents);
                    }
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(parse::Error::new(
                    ident.span(),
                    "expected one of: priority, resources or spawn",
                ));
            }
        }

        if content.is_empty() {
            break;
        }

        // ,
        let _: Token![,] = content.parse()?;
    }

    Ok(TaskArgs {
        capacity,
        priority: priority.unwrap_or(MIN_TASK_PRIORITY),
        resources: resources.unwrap_or(Idents::new()),
        spawn: spawn.unwrap_or(Idents::new()),
    })
}

pub struct Task {
    pub core: u8,
    pub args: TaskArgs,
    pub attrs: Vec<Attribute>,
    pub unsafety: Option<Token![unsafe]>,
    pub inputs: Vec<ArgCaptured>,
    pub statics: HashMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Task {
    fn check(args: TaskArgs, mut item: ItemFn, cores: u8) -> parse::Result<Self> {
        let valid_signature = item.vis == Visibility::Inherited
            && item.constness.is_none()
            && item.asyncness.is_none()
            && item.abi.is_none()
            && item.decl.generics.params.is_empty()
            && item.decl.generics.where_clause.is_none()
            && item.decl.variadic.is_none()
            && is_unit(&item.decl.output);

        let span = item.span();

        if !valid_signature {
            return Err(parse::Error::new(
                span,
                "`task` handlers must have type signature `[unsafe] fn(..)`",
            ));
        }

        let (statics, stmts) = extract_statics(item.block.stmts);

        let mut inputs = vec![];
        for input in item.decl.inputs {
            if let FnArg::Captured(capture) = input {
                inputs.push(capture);
            } else {
                return Err(parse::Error::new(
                    span,
                    "inputs must be named arguments (e.f. `foo: u32`) and not include `self`",
                ));
            }
        }

        match &*item.ident.to_string() {
            "init" | "idle" | "resources" => {
                return Err(parse::Error::new(
                    span,
                    "`task` handlers can NOT be named `idle`, `init` or `resources`",
                ));
            }
            _ => {}
        }

        let core = if cores == 1 {
            0
        } else {
            extract_cfg_core(&mut item.attrs, cores)?.ok_or_else(|| {
                parse::Error::new(
                    span,
                    "this item must be assigned to a single core \
                     (e.g. `#[cfg(core = \"0\")]`)",
                )
            })?
        };

        Ok(Task {
            core,
            args,
            attrs: item.attrs,
            unsafety: item.unsafety,
            inputs,
            statics: Static::parse(statics)?,
            stmts,
        })
    }
}

fn eq(attr: &Attribute, name: &str) -> bool {
    attr.style == AttrStyle::Outer && attr.path.segments.len() == 1 && {
        let pair = attr.path.segments.first().unwrap();
        let segment = pair.value();
        segment.arguments == PathArguments::None && segment.ident.to_string() == name
    }
}

/// Extracts `static mut` vars from the beginning of the given statements
fn extract_statics(stmts: Vec<Stmt>) -> (Statics, Vec<Stmt>) {
    let mut istmts = stmts.into_iter();

    let mut statics = Statics::new();
    let mut stmts = vec![];
    while let Some(stmt) = istmts.next() {
        match stmt {
            Stmt::Item(Item::Static(var)) => {
                if var.mutability.is_some() {
                    statics.push(var);
                } else {
                    stmts.push(Stmt::Item(Item::Static(var)));
                    break;
                }
            }
            _ => {
                stmts.push(stmt);
                break;
            }
        }
    }

    stmts.extend(istmts);

    (statics, stmts)
}

struct Cfg {
    ident: Ident,
    _eq: Token![=],
    str: LitStr,
}

impl Parse for Cfg {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let content;
        parenthesized!(content in input);
        Ok(Cfg {
            ident: content.parse()?,
            _eq: content.parse()?,
            str: content.parse()?,
        })
    }
}

fn extract_cfg_core(attrs: &mut Vec<Attribute>, cores: u8) -> parse::Result<Option<u8>> {
    for pos in attrs
        .iter()
        .enumerate()
        .filter_map(|(pos, attr)| if eq(attr, "cfg") { Some(pos) } else { None })
    {
        let cfg = &attrs[pos];

        if let Ok(cfg) = syn::parse2::<Cfg>(cfg.tts.clone()) {
            if cfg.ident.to_string() == "core" {
                if let Ok(i) = cfg.str.value().parse::<u8>() {
                    if i >= cores {
                        return Err(parse::Error::new(
                            cfg.str.span(),
                            &format!("integer must be in the range 0..={}", cores),
                        ));
                    } else {
                        attrs.swap_remove(pos);

                        return Ok(Some(i));
                    }
                } else {
                    return Err(parse::Error::new(
                        cfg.str.span(),
                        "value must be an integer",
                    ));
                }
            }
        }
    }

    Ok(None)
}

fn extract_assignments(stmts: Vec<Stmt>) -> (Vec<Stmt>, Vec<Assign>) {
    let mut istmts = stmts.into_iter().rev();

    let mut assigns = vec![];
    let mut stmts = vec![];
    while let Some(stmt) = istmts.next() {
        match stmt {
            Stmt::Semi(Expr::Assign(assign), semi) => {
                if let Expr::Path(ref expr) = *assign.left {
                    if expr.path.segments.len() == 1 {
                        assigns.push(Assign {
                            left: expr.path.segments[0].ident.clone(),
                            right: assign.right,
                        });
                        continue;
                    }
                }

                stmts.push(Stmt::Semi(Expr::Assign(assign), semi));
            }
            _ => {
                stmts.push(stmt);
                break;
            }
        }
    }

    stmts.extend(istmts);

    (stmts.into_iter().rev().collect(), assigns)
}

fn is_bottom(ty: &ReturnType) -> bool {
    if let ReturnType::Type(_, ty) = ty {
        if let Type::Never(_) = **ty {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn is_unit(ty: &ReturnType) -> bool {
    if let ReturnType::Type(_, ty) = ty {
        if let Type::Tuple(ref tuple) = **ty {
            tuple.elems.is_empty()
        } else {
            false
        }
    } else {
        true
    }
}
