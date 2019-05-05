use core::{
    sync::atomic::{AtomicU8, Ordering},
    u8,
};
use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Span;
use quote::quote;
use syn::{
    braced, bracketed, parenthesized,
    parse::{self, Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Brace,
    ArgCaptured, AttrStyle, Attribute, Expr, FnArg, Ident, IntSuffix, Item, ItemFn, ItemStatic,
    LitInt, Pat, PathArguments, ReturnType, Stmt, Token, Type, TypeTuple, Visibility,
};

use crate::PRIORITY_BITS;

static CORES: AtomicU8 = AtomicU8::new(1);

const MIN_PRIORITY: u8 = 1;
const MAX_PRIORITY: u8 = (1 << PRIORITY_BITS) - 1;

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
                    if val < 2 || val > 8 {
                        return Err(parse::Error::new(
                            lit.span(),
                            "number of cores must be in the range 2..=8",
                        ));
                    }

                    let val = val as u8;
                    CORES.store(val, Ordering::Relaxed);
                    cores = Some(val);
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
    pub ident: Ident,
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
            ident: input.parse()?,
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
    pub interrupts: Interrupts,
    pub tasks: Tasks,
}

pub struct Main {
    pub init: Option<Init>,
    pub idle: Option<Idle>,
}

#[derive(Debug)]
pub struct Spawn<'a> {
    // core that's sending this message
    pub core: u8,
    /// Priority of the spawner task
    // `None` means the spawner is `init`
    pub priority: Option<u8>,
    /// Task being spawned
    pub task: &'a Ident,
}

impl App {
    pub fn parse(items: Vec<Item>, args: AppArgs) -> parse::Result<Self> {
        let cores = args.cores;
        let mut idle = (0..cores).map(|_| None).collect::<Vec<_>>();
        let mut init = (0..cores).map(|_| None).collect::<Vec<_>>();
        let mut resources = BTreeMap::new();
        let mut interrupts = BTreeMap::new();
        let mut tasks = BTreeMap::new();

        for item in items {
            let span = item.span();
            match item {
                Item::Fn(mut item) => {
                    if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "idle")) {
                        let args: IdleArgs = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        let core = usize::from(args.core);
                        if idle[core].is_some() {
                            return Err(parse::Error::new(
                                span,
                                if cores == 1 {
                                    "`#[idle]` function must appear at most once"
                                } else {
                                    "an `#[idle]` function has already been assigned to this core"
                                },
                            ));
                        }

                        idle[core] = Some(Idle::check(args, item)?);
                    } else if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "init")) {
                        let args: InitArgs = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        let core = usize::from(args.core);
                        if init[core].is_some() {
                            return Err(parse::Error::new(
                                span,
                                if cores == 1 {
                                    "`#[init]` function must appear at most once"
                                } else {
                                    "an `#[init]` function has already been assigned to this core"
                                },
                            ));
                        }

                        init[core] = Some(Init::check(args, item)?);
                    } else if let Some(pos) =
                        item.attrs.iter().position(|attr| eq(attr, "interrupt"))
                    {
                        if interrupts.contains_key(&item.ident) {
                            return Err(parse::Error::new(
                                item.ident.span(),
                                "this interrupt is defined multiple times",
                            ));
                        }

                        let args = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        interrupts.insert(item.ident.clone(), Interrupt::check(args, item)?);
                    } else if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "task")) {
                        if tasks.contains_key(&item.ident) {
                            return Err(parse::Error::new(
                                item.ident.span(),
                                "this task is defined multiple times",
                            ));
                        }

                        let args = syn::parse2(item.attrs.swap_remove(pos).tts)?;

                        tasks.insert(item.ident.clone(), Task::check(args, item)?);
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

                    resources.insert(item.ident.clone(), Resource::check(item)?);
                }

                _ => {
                    return Err(parse::Error::new(
                        span,
                        "this item must live outside the `#[app]` module",
                    ));
                }
            }
        }

        let mains = (0..cores)
            .map(|core| {
                let core = usize::from(core);
                Main {
                    init: init[core].take(),
                    idle: idle[core].take(),
                }
            })
            .collect();

        Ok(App {
            cores,
            interrupts,
            mains,
            resources,
            tasks,
        })
    }

    /// Returns an iterator over all resource accesses.
    ///
    /// Each resource access include the priority it's accessed at (`u8`) and the name of the
    /// resource (`Ident`). A resource may appear more than once in this iterator
    pub fn resource_accesses(
        &self,
    ) -> impl Iterator<
        Item = (
            /* core: */ u8,
            /* priority: */ Option<u8>, // `None` means `init`
            /* resource: */ &Ident,
        ),
    > {
        self.mains
            .iter()
            .zip(0..)
            .flat_map(|(main, core)| {
                main.init
                    .iter()
                    .flat_map(move |init| {
                        init.args.resources.iter().map(move |res| (core, None, res))
                    })
                    .chain(main.idle.iter().flat_map(move |idle| {
                        idle.args
                            .resources
                            .iter()
                            .map(move |res| (core, Some(0), res))
                    }))
            })
            .chain(self.tasks.values().flat_map(|task| {
                task.args
                    .resources
                    .iter()
                    .map(move |res| (task.args.core, Some(task.args.priority), res))
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
            .flat_map(move |(main, core)| {
                main.init
                    .iter()
                    .flat_map(move |init| {
                        init.args.spawn.iter().map(move |task| Spawn {
                            core,
                            priority: None,
                            task,
                        })
                    })
                    .chain(main.idle.iter().flat_map(move |idle| {
                        idle.args.spawn.iter().map(move |task| Spawn {
                            core,
                            priority: Some(0),
                            task,
                        })
                    }))
            })
            .chain(self.interrupts.values().flat_map(move |interrupt| {
                interrupt.args.spawn.iter().map(move |callee| Spawn {
                    core: interrupt.args.core,
                    priority: Some(interrupt.args.priority),
                    task: callee,
                })
            }))
            .chain(self.tasks.values().flat_map(move |task| {
                task.args.spawn.iter().map(move |callee| Spawn {
                    core: task.args.core,
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
                main.init
                    .iter()
                    .map(move |init| {
                        (
                            core,
                            Ident::new("init", Span::call_site()),
                            &init.args.spawn,
                        )
                    })
                    .chain(main.idle.iter().map(move |idle| {
                        (
                            core,
                            Ident::new("idle", Span::call_site()),
                            &idle.args.spawn,
                        )
                    }))
            })
            .chain(self.interrupts.iter().map(|(name, interrupt)| {
                (interrupt.args.core, name.clone(), &interrupt.args.spawn)
            }))
            .chain(
                self.tasks
                    .iter()
                    .map(|(name, task)| (task.args.core, name.clone(), &task.args.spawn)),
            )
    }

    pub fn cfg_core(&self, core: u8) -> Option<proc_macro2::TokenStream> {
        if self.cores == 1 {
            None
        } else {
            let core = core.to_string();
            Some(quote!(#[cfg(core = #core)]))
        }
    }
}

pub type Idents = BTreeSet<Ident>;
pub type Resources = BTreeMap<Ident, Resource>;
pub type Statics = Vec<ItemStatic>;
pub type Interrupts = BTreeMap<Ident, Interrupt>;
pub type Tasks = BTreeMap<Ident, Task>;

pub struct Static {
    pub attrs: Vec<Attribute>,
    pub cfgs: Vec<Attribute>,
    pub expr: Box<Expr>,
    pub global: bool,
    pub ty: Box<Type>,
}

impl Static {
    fn parse(items: Vec<ItemStatic>) -> parse::Result<BTreeMap<Ident, Static>> {
        let mut statics = BTreeMap::new();

        for item in items {
            if statics.contains_key(&item.ident) {
                return Err(parse::Error::new(
                    item.ident.span(),
                    "this `static` is listed twice",
                ));
            }

            let (cfgs, attrs) = extract_cfgs(item.attrs);
            let (global, attrs) = extract_global(attrs);

            statics.insert(
                item.ident,
                Static {
                    attrs,
                    cfgs,
                    expr: item.expr,
                    global,
                    ty: item.ty,
                },
            );
        }

        Ok(statics)
    }
}

pub struct InitArgs {
    pub core: u8,
    pub late: Idents,
    pub resources: Idents,
    pub spawn: Idents,
}

impl Default for InitArgs {
    fn default() -> Self {
        debug_assert_eq!(CORES.load(Ordering::Relaxed), 1, "BUG");

        InitArgs {
            core: 0,
            late: Idents::new(),
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

impl Parse for InitArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        parse_init_idle_args(input, CORES.load(Ordering::Relaxed), true)
    }
}

fn parse_init_idle_args(
    input: ParseStream,
    cores: u8,
    accepts_late: bool,
) -> parse::Result<InitArgs> {
    if input.is_empty() {
        if cores == 1 {
            return Ok(InitArgs::default());
        } else {
            return Err(parse::Error::new(
                Span::call_site(),
                if accepts_late {
                    "all `#[init]` functions must specify the core they'll run on"
                } else {
                    "all `#[idle]` functions must specify the core they'll run on"
                },
            ));
        }
    }

    let cores = CORES.load(Ordering::Relaxed);
    let mut core = None;
    let mut late = None;
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
            "core" if cores != 1 => {
                if core.is_some() {
                    return Err(parse::Error::new(
                        ident.span(),
                        "argument appears more than once",
                    ));
                }

                let lit: LitInt = content.parse()?;
                core = Some(check_core(lit, cores)?);
            }

            "late" if accepts_late => {
                if late.is_some() {
                    return Err(parse::Error::new(
                        ident.span(),
                        "argument appears more than once",
                    ));
                }

                let idents = parse_idents(&content)?;

                late = Some(idents);
            }

            "resources" | "spawn" => {
                let idents = parse_idents(&content)?;

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
            }

            _ => {
                return Err(parse::Error::new(ident.span(), "unexpected argument"));
            }
        }

        if content.is_empty() {
            break;
        }

        // ,
        let _: Token![,] = content.parse()?;
    }

    Ok(InitArgs {
        core: if cores == 1 {
            0
        } else {
            core.ok_or_else(|| {
                parse::Error::new(
                    Span::call_site(),
                    &format!(
                        "all `#[{}]` functions must be assigned to a core",
                        if accepts_late { "init" } else { "idle" }
                    ),
                )
            })?
        },
        late: late.unwrap_or(Idents::new()),
        resources: resources.unwrap_or(Idents::new()),
        spawn: spawn.unwrap_or(Idents::new()),
    })
}

pub struct Init {
    pub args: InitArgs,
    pub attrs: Vec<Attribute>,
    pub context: Pat,
    pub returns_late_resources: bool,
    pub span: Span,
    pub statics: BTreeMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Init {
    fn check(args: InitArgs, item: ItemFn) -> parse::Result<Self> {
        let mut valid_signature = check_signature(&item) && item.decl.inputs.len() == 1;

        const DONT_CARE: bool = false;

        let returns_late_resources = match &item.decl.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ty) => {
                match &**ty {
                    Type::Tuple(t) => {
                        if t.elems.is_empty() {
                            // -> ()
                            true
                        } else {
                            valid_signature = false;

                            DONT_CARE
                        }
                    }

                    Type::Path(_) => {
                        if is_path(ty, &["init", "LateResources"]) {
                            // -> init::LateResources
                            true
                        } else {
                            valid_signature = false;

                            DONT_CARE
                        }
                    }

                    _ => {
                        valid_signature = false;

                        DONT_CARE
                    }
                }
            }
        };

        let span = item.span();

        if valid_signature {
            if let Some((context, _)) = check_inputs(item.decl.inputs, "init") {
                let (statics, stmts) = extract_statics(item.block.stmts);

                return Ok(Init {
                    args,
                    attrs: item.attrs,
                    statics: Static::parse(statics)?,
                    context,
                    stmts,
                    returns_late_resources,
                    span,
                });
            }
        }

        Err(parse::Error::new(
            span,
            "`init` must have type signature `fn(init::Context) [-> init::LateResources]`",
        ))
    }
}

pub struct IdleArgs {
    pub core: u8,
    pub resources: Idents,
    pub spawn: Idents,
}

impl Parse for IdleArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        parse_init_idle_args(input, CORES.load(Ordering::Relaxed), false).map(|args| IdleArgs {
            core: args.core,
            resources: args.resources,
            spawn: args.spawn,
        })
    }
}

pub struct Idle {
    pub args: IdleArgs,
    pub attrs: Vec<Attribute>,
    pub context: Pat,
    pub statics: BTreeMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Idle {
    fn check(args: IdleArgs, item: ItemFn) -> parse::Result<Self> {
        let valid_signature =
            check_signature(&item) && item.decl.inputs.len() == 1 && is_bottom(&item.decl.output);

        let span = item.span();

        if valid_signature {
            if let Some((context, _)) = check_inputs(item.decl.inputs, "idle") {
                let (statics, stmts) = extract_statics(item.block.stmts);

                return Ok(Idle {
                    args,
                    attrs: item.attrs,
                    context,
                    statics: Static::parse(statics)?,
                    stmts,
                });
            }
        }

        Err(parse::Error::new(
            span,
            "`idle` must have type signature `fn(idle::Context) -> !`",
        ))
    }
}

pub struct Resource {
    pub attrs: Vec<Attribute>,
    pub cfgs: Vec<Attribute>,
    pub expr: Option<Box<Expr>>,
    pub mutability: Option<Token![mut]>,
    pub ty: Box<Type>,
}

impl Resource {
    fn check(item: ItemStatic) -> parse::Result<Resource> {
        let span = item.span();

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

        let (cfgs, attrs) = extract_cfgs(item.attrs);

        Ok(Resource {
            attrs,
            cfgs,
            expr: if uninitialized { None } else { Some(item.expr) },
            mutability: item.mutability,
            ty: item.ty,
        })
    }
}

/// Union of `TaskArgs`, `ExceptionArgs` and `InterruptArgs`
pub struct Args {
    pub binds: Option<Ident>,
    pub capacity: Option<u8>,
    pub core: u8,
    pub priority: u8,
    pub resources: Idents,
    pub spawn: Idents,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            binds: None,
            capacity: None,
            core: 0,
            priority: 1,
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

pub struct InterruptArgs {
    binds: Option<Ident>,
    pub core: u8,
    pub priority: u8,
    pub resources: Idents,
    pub spawn: Idents,
}

impl InterruptArgs {
    /// Returns the name of the exception / interrupt this handler binds to
    pub fn binds<'a>(&'a self, handler: &'a Ident) -> &'a Ident {
        self.binds.as_ref().unwrap_or(handler)
    }
}

impl Default for InterruptArgs {
    fn default() -> Self {
        debug_assert_eq!(CORES.load(Ordering::Relaxed), 1, "BUG");

        InterruptArgs {
            binds: None,
            core: 0,
            priority: MIN_PRIORITY,
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

impl Parse for InterruptArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let cores = CORES.load(Ordering::Relaxed);

        parse_interrupt_or_task_args(input, cores, true, false).map(|args| InterruptArgs {
            binds: args.binds,
            core: args.core,
            priority: args.priority,
            resources: args.resources,
            spawn: args.spawn,
        })
    }
}

pub struct Interrupt {
    pub args: InterruptArgs,
    pub attrs: Vec<Attribute>,
    pub context: Pat,
    pub statics: BTreeMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Interrupt {
    fn check(args: InterruptArgs, item: ItemFn) -> parse::Result<Self> {
        let valid_signature =
            check_signature(&item) && item.decl.inputs.len() == 1 && is_unit(&item.decl.output);

        let span = item.span();

        let name = item.ident.to_string();
        if valid_signature {
            if let Some((context, _)) = check_inputs(item.decl.inputs, &name) {
                match &*name {
                    "init" | "idle" | "resources" => {
                        return Err(parse::Error::new(
                            span,
                            "`interrupt` handlers can NOT be named `idle`, `init` or `resources`",
                        ));
                    }
                    _ => {}
                }

                let (statics, stmts) = extract_statics(item.block.stmts);

                return Ok(Interrupt {
                    args,
                    attrs: item.attrs,
                    statics: Static::parse(statics)?,
                    context,
                    stmts,
                });
            }
        }

        Err(parse::Error::new(
            span,
            format!(
                "this `interrupt` handler must have type signature `fn({}::Context)`",
                name
            ),
        ))
    }
}

pub struct TaskArgs {
    pub capacity: u8,
    pub core: u8,
    pub priority: u8,
    pub resources: Idents,
    pub spawn: Idents,
}

impl Default for TaskArgs {
    fn default() -> Self {
        debug_assert_eq!(CORES.load(Ordering::Relaxed), 1, "BUG");

        TaskArgs {
            capacity: 1,
            core: 0,
            priority: MIN_PRIORITY,
            resources: Idents::new(),
            spawn: Idents::new(),
        }
    }
}

impl Parse for TaskArgs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let cores = CORES.load(Ordering::Relaxed);

        parse_interrupt_or_task_args(input, cores, false, true).map(|args| TaskArgs {
            capacity: args.capacity.unwrap_or(1),
            core: args.core,
            priority: args.priority,
            resources: args.resources,
            spawn: args.spawn,
        })
    }
}

// Parser shared by TaskArgs and InterruptArgs
fn parse_interrupt_or_task_args(
    input: ParseStream,
    cores: u8,
    accepts_binds: bool,
    accepts_capacity: bool,
) -> parse::Result<Args> {
    const ERR_MSG: &str = "all tasks must specify the core they'll run on";

    if input.is_empty() {
        if cores == 1 {
            return Ok(Args::default());
        } else {
            return Err(parse::Error::new(Span::call_site(), ERR_MSG));
        }
    }

    let mut binds = None;
    let mut capacity = None;
    let mut core = None;
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
            "binds" if accepts_binds => {
                if binds.is_some() {
                    return Err(parse::Error::new(
                        ident.span(),
                        "argument appears more than once",
                    ));
                }

                // #ident
                let ident = content.parse()?;

                binds = Some(ident);
            }

            "capacity" if accepts_capacity => {
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

            "core" if cores != 1 => {
                if core.is_some() {
                    return Err(parse::Error::new(
                        ident.span(),
                        "argument appears more than once",
                    ));
                }

                let lit: LitInt = content.parse()?;
                core = Some(check_core(lit, cores)?);
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
                if value > u64::from(MAX_PRIORITY) || value < u64::from(MIN_PRIORITY) {
                    return Err(parse::Error::new(
                        lit.span(),
                        format!(
                            "this literal must be in the range {}..={}",
                            MIN_PRIORITY, MAX_PRIORITY
                        ),
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

    Ok(Args {
        binds,
        capacity,
        core: if cores == 1 {
            0
        } else {
            core.ok_or_else(|| parse::Error::new(Span::call_site(), ERR_MSG))?
        },
        priority: priority.unwrap_or(MIN_PRIORITY),
        resources: resources.unwrap_or(Idents::new()),
        spawn: spawn.unwrap_or(Idents::new()),
    })
}

pub struct Task {
    pub args: TaskArgs,
    pub attrs: Vec<Attribute>,
    pub cfgs: Vec<Attribute>,
    pub context: Pat,
    pub inputs: Vec<ArgCaptured>,
    pub statics: BTreeMap<Ident, Static>,
    pub stmts: Vec<Stmt>,
}

impl Task {
    fn check(args: TaskArgs, item: ItemFn) -> parse::Result<Self> {
        let valid_signature =
            check_signature(&item) && !item.decl.inputs.is_empty() && is_unit(&item.decl.output);

        let span = item.span();

        let name = item.ident.to_string();
        if valid_signature {
            if let Some((context, rest)) = check_inputs(item.decl.inputs, &name) {
                let (statics, stmts) = extract_statics(item.block.stmts);

                let inputs = rest.map_err(|arg| {
                    parse::Error::new(
                        arg.span(),
                        "inputs must be named arguments (e.f. `foo: u32`) and not include `self`",
                    )
                })?;

                match &*name {
                    "init" | "idle" | "resources" => {
                        return Err(parse::Error::new(
                            span,
                            "`task` handlers can NOT be named `idle`, `init` or `resources`",
                        ));
                    }
                    _ => {}
                }

                let (cfgs, attrs) = extract_cfgs(item.attrs);
                return Ok(Task {
                    args,
                    cfgs,
                    attrs,
                    inputs,
                    context,
                    statics: Static::parse(statics)?,
                    stmts,
                });
            }
        }

        Err(parse::Error::new(
            span,
            &format!(
                "this `task` handler must have type signature `fn({}::Context, ..)`",
                name
            ),
        ))
    }
}

fn eq(attr: &Attribute, name: &str) -> bool {
    attr.style == AttrStyle::Outer && attr.path.segments.len() == 1 && {
        let pair = attr.path.segments.first().unwrap();
        let segment = pair.value();
        segment.arguments == PathArguments::None && segment.ident.to_string() == name
    }
}

fn extract_cfgs(attrs: Vec<Attribute>) -> (Vec<Attribute>, Vec<Attribute>) {
    let mut cfgs = vec![];
    let mut not_cfgs = vec![];

    for attr in attrs {
        if eq(&attr, "cfg") {
            cfgs.push(attr);
        } else {
            not_cfgs.push(attr);
        }
    }

    (cfgs, not_cfgs)
}

fn extract_global(mut attrs: Vec<Attribute>) -> (bool, Vec<Attribute>) {
    if let Some(pos) = attrs.iter().position(|attr| eq(attr, "global")) {
        attrs.swap_remove(pos);
        (true, attrs)
    } else {
        (false, attrs)
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

//  [#(#idents)*]
fn parse_idents(content: ParseStream) -> parse::Result<Idents> {
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

    Ok(idents)
}

fn check_core(lit: LitInt, cores: u8) -> parse::Result<u8> {
    if lit.suffix() != IntSuffix::None {
        return Err(parse::Error::new(
            lit.span(),
            "this integer must be unsuffixed",
        ));
    }

    let val = lit.value();
    if val >= u64::from(cores) {
        return Err(parse::Error::new(
            lit.span(),
            &format!("number of cores must be in the range 0..{}", cores),
        ));
    }

    Ok(val as u8)
}

// checks that the list of arguments has the form `#pat: #name::Context, (..)`
//
// if the check succeeds it returns `#pat` plus the remaining arguments
fn check_inputs(
    inputs: Punctuated<FnArg, Token![,]>,
    name: &str,
) -> Option<(Pat, Result<Vec<ArgCaptured>, FnArg>)> {
    let mut inputs = inputs.into_iter();

    match inputs.next() {
        Some(FnArg::Captured(first)) => {
            if is_path(&first.ty, &[name, "Context"]) {
                let rest = inputs
                    .map(|arg| match arg {
                        FnArg::Captured(arg) => Ok(arg),
                        _ => Err(arg),
                    })
                    .collect::<Result<Vec<_>, _>>();

                Some((first.pat, rest))
            } else {
                None
            }
        }

        _ => None,
    }
}

/// checks that a function signature
///
/// - has no bounds (like where clauses)
/// - is not `async`
/// - is not `const`
/// - is not `unsafe`
/// - is not generic (has no type parametrs)
/// - is not variadic
/// - uses the Rust ABI (and not e.g. "C")
fn check_signature(item: &ItemFn) -> bool {
    item.vis == Visibility::Inherited
        && item.constness.is_none()
        && item.asyncness.is_none()
        && item.abi.is_none()
        && item.unsafety.is_none()
        && item.decl.generics.params.is_empty()
        && item.decl.generics.where_clause.is_none()
        && item.decl.variadic.is_none()
}

fn is_path(ty: &Type, segments: &[&str]) -> bool {
    match ty {
        Type::Path(tpath) if tpath.qself.is_none() => {
            tpath.path.segments.len() == segments.len()
                && tpath
                    .path
                    .segments
                    .iter()
                    .zip(segments)
                    .all(|(lhs, rhs)| lhs.ident == **rhs)
        }

        _ => false,
    }
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
