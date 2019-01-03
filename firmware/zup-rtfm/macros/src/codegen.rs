use proc_macro::TokenStream;
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use proc_macro2::Span;
use quote::quote;
use rand::{Rng, SeedableRng};
use syn::{ArgCaptured, Ident, IntSuffix, LitInt};

use crate::{
    analyze::{Analysis, Ownership},
    syntax::{App, Idents, Idle, Init, Static},
    PRIORITY_BITS,
};

// NOTE to avoid polluting the user namespaces we map some identifiers to pseudo-hygienic names.
// In some instances we also use the pseudo-hygienic names for safety, for example the user should
// not modify the priority field of resources.
type Aliases = HashMap<Ident, Ident>;

struct Context {
    dispatchers: Vec<BTreeMap<u8, Dispatcher>>,
    // Alias (`fn`)
    idle: Ident,
    // Alias (`fn`)
    init: Ident,
    // Alias
    priority: Ident,
    /// Task -> Alias (`struct`)
    resources: HashMap<Kind, Resources>,
    // For non-singletons this maps the resource name to its `static mut` variable name
    statics: Aliases,
    tasks: HashMap<Ident, Task>,
}

struct Dispatcher {
    enum_: Ident,
    ready_queue: Ident,
}

struct Task {
    alias: Ident,
    local: Option<Spawn>,
    shared: Option<Spawn>,
}

struct Spawn {
    f: Ident,
    inputs: Ident,
    free_queue: Ident,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            dispatchers: vec![],
            idle: mk_ident(Some("idle")),
            init: mk_ident(Some("init")),
            priority: mk_ident(None),
            resources: HashMap::new(),
            statics: Aliases::new(),
            tasks: HashMap::new(),
        }
    }
}

struct Resources {
    alias: Ident,
    decl: proc_macro2::TokenStream,
}

pub fn app(app: &App, analysis: &Analysis) -> TokenStream {
    let mut ctxt = Context::default();

    let mut items = vec![];

    items.push(resources(&mut ctxt, &app, analysis));

    items.push(tasks(&mut ctxt, &app, analysis));

    items.push(dispatchers(&mut ctxt, &app, analysis));

    items.push(spawn(&mut ctxt, app, analysis));

    for (main, core) in app.mains.iter().zip(0..) {
        let cfg = mk_cfg(app.cores, Some(core));

        let pre_init = pre_init(&ctxt, core, app, analysis);
        items.push(init(&mut ctxt, &cfg, &main.init, &app, analysis));

        let (idle_fn, idle_expr) = idle(&mut ctxt, &cfg, &main.idle, &app, analysis);
        items.push(idle_fn);

        let init = &ctxt.init;
        items.push(quote!(
            #[allow(unsafe_code)]
            #[doc(hidden)]
            #[rtfm::export::entry]
            #cfg
            unsafe fn main() -> ! {
                #pre_init

                #init();

                rtfm::export::enable_irq();

                #idle_expr
            }
        ))
    }

    if app.cores == 1 {
        quote!(#(#items)*).into()
    } else {
        quote!(
            #[rtfm::export::amp]
            const AMP: () = {
                #(#items)*
            };
        )
        .into()
    }
}

fn resources(ctxt: &mut Context, app: &App, analysis: &Analysis) -> proc_macro2::TokenStream {
    let mut items = vec![];
    let mut modules = (0..app.cores).map(|_| vec![]).collect::<Vec<_>>();

    for (name, res) in &app.resources {
        if let Some(core) = res.core {
            let cfg = mk_cfg(app.cores, Some(core));

            let attrs = &res.attrs;
            let ty = &res.ty;
            let expr = &res.expr;

            let alias = mk_ident(None);
            let symbol = format!("{}::{}", name, alias);

            items.push(
                expr.as_ref()
                    .map(|expr| {
                        quote!(
                            #(#attrs)*
                            #[doc = #symbol]
                            #cfg
                            static mut #alias: #ty = #expr;
                        )
                    })
                    .unwrap_or_else(|| {
                        quote!(
                            #(#attrs)*
                            #[doc = #symbol]
                            #cfg
                            static mut #alias: rtfm::export::MaybeUninit<#ty> =
                                rtfm::export::MaybeUninit::uninitialized();
                        )
                    }),
            );

            if let Some(Ownership::Shared { ceiling }) = analysis.ownerships.get(name) {
                // NOTE `#[shared]` statics (`core = None`) are not resources
                if res.mutability.is_some() && res.core.is_some() {
                    let ptr = if res.expr.is_none() {
                        quote!(unsafe { #alias.get_mut() })
                    } else {
                        quote!(unsafe { &mut #alias })
                    };

                    items.push(mk_resource(
                        ctxt,
                        &cfg,
                        name,
                        quote!(#ty),
                        *ceiling,
                        ptr,
                        Some(&mut modules[usize::from(core)]),
                    ));
                }
            }

            ctxt.statics.insert(name.clone(), alias);
        } else {
            // TODO implement something here?
        }
    }

    for (module, core) in modules.into_iter().zip(0..) {
        if !module.is_empty() {
            let cfg = mk_cfg(app.cores, Some(core));

            items.push(quote!(
                /// Resource proxies
                #cfg
                pub mod resources {
                    #(#module)*
                }
            ));
        }
    }

    quote!(#(#items)*)
}

fn tasks(ctxt: &mut Context, app: &App, analysis: &Analysis) -> proc_macro2::TokenStream {
    let mut items = vec![];

    // first pass to generate buffers (statics and resources) and spawn aliases
    for (name, task) in &app.tasks {
        let cfg = mk_cfg(app.cores, Some(task.core));

        let inputs = &task.inputs;
        let ty = tuple_ty(inputs);

        let task_ = &analysis.tasks[name];
        let mut local = None;
        if task_.local.capacity != 0 {
            let free_queue = mk_ident(None);
            let inputs = mk_ident(None);

            let cap_lit = mk_capacity_literal(task_.local.capacity);
            let cap_ty = mk_typenum_capacity(task_.local.capacity, true);

            let resource = mk_resource(
                ctxt,
                &cfg,
                &free_queue,
                quote!(rtfm::export::FreeQueue<#cap_ty>),
                task_.local.ceiling,
                quote!(&mut #free_queue),
                None,
            );

            let i_symbol = format!("{}::LOCAL_INPUTS::{}", name, inputs);
            let fq_symbol = format!("{}::LOCAL_FREE_QUEUE::{}", name, free_queue);
            items.push(quote!(
                #[doc = #i_symbol]
                #cfg
                static mut #inputs: rtfm::export::MaybeUninit<[#ty; #cap_lit]> =
                    rtfm::export::MaybeUninit::uninitialized();

                #[doc = #fq_symbol]
                #cfg
                static mut #free_queue: rtfm::export::FreeQueue<#cap_ty>
                    = rtfm::export::FreeQueue::new();

                #resource
            ));

            local = Some(Spawn {
                f: mk_ident(None),
                inputs,
                free_queue,
            })
        }

        let mut shared = None;
        if task_.shared.capacity != 0 {
            // these need to have consistent names across compilations or `#[shared]` won't work
            // let free_queue = mk_ident(None);
            // let inputs = mk_ident(None);
            let free_queue = Ident::new(&format!("{}_SHARED_FREE_QUEUE", name), Span::call_site());
            let inputs = Ident::new(&format!("{}_SHARED_INPUTS", name), Span::call_site());

            let cap_lit = mk_capacity_literal(task_.shared.capacity);
            let cap_ty = mk_typenum_capacity(task_.shared.capacity, true);

            let resource = mk_resource(
                ctxt,
                &None,
                &free_queue,
                quote!(rtfm::export::FreeQueue<#cap_ty>),
                task_.local.ceiling,
                quote!(#free_queue.get_mut()),
                None,
            );

            let i_symbol = format!("{}::SHARED_INPUTS::{}", name, inputs);
            let fq_symbol = format!("{}::SHARED_FREE_QUEUE::{}", name, free_queue);
            items.push(quote!(
                #[doc = #i_symbol]
                #[shared]
                static mut #inputs: [#ty; #cap_lit] = ();

                #[doc = #fq_symbol]
                #[shared]
                static mut #free_queue: rtfm::export::FreeQueue<#cap_ty> = ();

                #resource
            ));

            shared = Some(Spawn {
                f: mk_ident(None),
                inputs,
                free_queue,
            })
        }

        let alias = mk_ident(None);
        ctxt.tasks.insert(
            name.clone(),
            Task {
                alias,
                local,
                shared,
            },
        );
    }

    // second pass to generate the actual task function
    for (name, task) in &app.tasks {
        let cfg = mk_cfg(app.cores, Some(task.core));

        let prelude = prelude(
            ctxt,
            &cfg,
            Kind::Task(name.clone()),
            &task.args.resources,
            &task.args.spawn,
            app,
            task.args.priority,
            analysis,
        );

        // NOTE `module` needs to be called after `prelude`
        items.push(module(
            ctxt,
            &cfg,
            Kind::Task(name.clone()),
            !task.args.spawn.is_empty(),
        ));

        let alias = &ctxt.tasks[name].alias;
        let inputs = &task.inputs;
        let locals = mk_locals(&task.statics, false);
        let stmts = &task.stmts;
        let attrs = &task.attrs;
        let unsafety = &task.unsafety;
        items.push(quote!(
            #(#attrs)*
            #cfg
            #unsafety fn #alias(#(#inputs,)*) {
                #(#locals)*

                #prelude

                #(#stmts)*
            }
        ));
    }

    quote!(#(#items)*)
}

fn dispatchers(ctxt: &mut Context, app: &App, analysis: &Analysis) -> proc_macro2::TokenStream {
    let mut items = vec![];

    ctxt.dispatchers = (0..app.cores).map(|_| BTreeMap::new()).collect();
    for (core, dispatchers) in analysis.dispatchers.iter().enumerate() {
        let cfg = mk_cfg(app.cores, Some(core as u8));

        for (level, dispatcher) in dispatchers {
            let enum_ = mk_ident(None);

            let mut shared = false;
            let variants = &dispatcher
                .tasks
                .iter()
                .map(|(task, cross)| {
                    Ident::new(
                        &if *cross {
                            shared = true;
                            format!("{}X", task)
                        } else {
                            task.to_string()
                        },
                        Span::call_site(),
                    )
                })
                .collect::<Vec<_>>();

            let cfg_ = if shared { &None } else { &cfg };

            let cap = mk_typenum_capacity(dispatcher.capacity, true);

            let e = quote!(rtfm::export);
            let ty = quote!(#e::ReadyQueue<#enum_, #cap>);
            let ceiling = analysis.dispatchers[core][level].ceiling;

            let ready_queue;
            let refmut;
            if shared {
                // this needs have consistent names across compilations or `#[shared]` won't work
                ready_queue = Ident::new(
                    &format!("C{}P{}_READY_QUEUE", core, level),
                    Span::call_site(),
                );

                refmut = quote!(#ready_queue.get_mut());

                items.push(quote!(
                    #[shared]
                    static mut #ready_queue: #ty = ();
                ));
            } else {
                ready_queue = mk_ident(None);

                refmut = quote!(&mut #ready_queue);

                items.push(quote!(
                    #cfg
                    static mut #ready_queue: #ty = #e::ReadyQueue::new();
                ));
            };

            let resource = mk_resource(
                ctxt,
                cfg_,
                &ready_queue,
                ty.clone(),
                ceiling,
                refmut.clone(),
                None,
            );

            items.push(quote!(
                #[allow(dead_code)]
                #[allow(non_camel_case_types)]
                #cfg_
                enum #enum_ { #(#variants,)* }

                #resource
            ));

            let sgi = Ident::new(&format!("SG{}", dispatcher.sgi), Span::call_site());
            let arms = dispatcher
                .tasks
                .iter()
                .enumerate()
                .map(|(i, (task, cross))| {
                    let variant = &variants[i];
                    let task_ = &ctxt.tasks[task];

                    let message;
                    let free_queue;
                    if *cross {
                        message = task_.shared.as_ref().expect("unreachable");
                        let fq = &message.free_queue;
                        free_queue = quote!(#fq.get_mut());
                    } else {
                        message = task_.local.as_ref().expect("unreachable");
                        let fq = &message.free_queue;
                        free_queue = quote!(#fq);
                    };

                    let inputs = &message.inputs;
                    let pats = tuple_pat(&app.tasks[task].inputs);
                    let alias = &task_.alias;
                    let call = quote!(#alias(#pats));

                    quote!(#enum_::#variant => {
                        let input = ptr::read(#inputs.get_ref().get_unchecked(usize::from(index)));
                        #free_queue.split().0.enqueue_unchecked(index);
                        let (#pats) = input;
                        #call
                    })
                })
                .collect::<Vec<_>>();

            items.push(quote!(
                #[rtfm::export::interrupt]
                #cfg
                unsafe fn #sgi() {
                    use core::ptr;

                    rtfm::export::run(|| {
                        while let Some((task, index)) = (#refmut).split().1.dequeue() {
                            match task {
                                #(#arms)*
                            }
                        }
                    });
                }
            ));

            ctxt.dispatchers[core].insert(*level, Dispatcher { enum_, ready_queue });
        }
    }

    quote!(#(#items)*)
}

fn spawn(ctxt: &Context, app: &App, analysis: &Analysis) -> proc_macro2::TokenStream {
    let mut items = vec![];

    // Generate `spawn` functions
    let priority = &ctxt.priority;
    for (name, task) in &ctxt.tasks {
        let task_ = &app.tasks[name];
        let core = usize::from(task_.core);
        let level = task_.args.priority;
        let args = &task_.inputs;
        let ty = tuple_ty(args);
        let pats = tuple_pat(args);
        let dispatcher = &ctxt.dispatchers[core][&level];
        let rq = &dispatcher.ready_queue;
        let enum_ = &dispatcher.enum_;
        let sgi = analysis.dispatchers[core][&level].sgi;
        for (cross, spawn) in task
            .local
            .iter()
            .map(|local| (false, local))
            .chain(task.shared.iter().map(|shared| (true, shared)))
        {
            let alias = &spawn.f;
            let fq = &spawn.free_queue;
            let inputs = &spawn.inputs;
            let variant = Ident::new(
                &if cross {
                    format!("{}X", name)
                } else {
                    name.to_string()
                },
                Span::call_site(),
            );

            let cfg = mk_cfg(app.cores, if cross { None } else { Some(core as u8) });
            let core = if cross {
                let core = core as u8;
                quote!(Some(#core))
            } else {
                quote!(None)
            };
            items.push(quote!(
                #[inline(always)]
                #cfg
                unsafe fn #alias(
                    #priority: &core::cell::Cell<u8>,
                    #(#args,)*
                ) -> Result<(), #ty> {
                    use core::ptr;

                    use rtfm::Mutex;

                    if let Some(index) = (#fq { #priority }).lock(|f| f.split().1.dequeue()) {
                        ptr::write(#inputs.get_mut().get_unchecked_mut(usize::from(index)), (#pats));

                        #rq { #priority }.lock(|rq| {
                            rq.split().0.enqueue_unchecked((#enum_::#variant, index))
                        });

                        rtfm::export::sgi(#sgi, #core);

                        Ok(())
                    } else {
                        Err((#pats))
                    }
                }
            ));
        }
    }

    // Generate `spawn` structs; these call the `spawn` functions generated above
    for (caller, name, spawn) in app.spawn_callers() {
        if spawn.is_empty() {
            continue;
        }

        let cfg = mk_cfg(app.cores, Some(caller));
        let mut methods = vec![];
        for task in spawn {
            let task_ = &app.tasks[task];
            let callee = task_.core;
            let inputs = &task_.inputs;
            let ty = tuple_ty(inputs);
            let pats = tuple_pat(inputs);
            let alias = if caller == callee {
                // local
                &ctxt.tasks[task].local.as_ref().expect("unreachable").f
            } else {
                // cross-core
                &ctxt.tasks[task].shared.as_ref().expect("unreachable").f
            };

            methods.push(quote!(
                #[allow(unsafe_code)]
                #[inline]
                pub fn #task(&self, #(#inputs,)*) -> Result<(), #ty> {
                    unsafe { #alias(&self.#priority, #pats) }
                }
            ));
        }

        items.push(quote!(
            #cfg
            impl<'a> #name::Spawn<'a> {
                #(#methods)*
            }
        ));
    }

    quote!(#(#items)*)
}

/// This function creates creates a module for `init` / `idle` / a `task` (see `kind` argument)
fn module(
    ctxt: &mut Context,
    cfg: &Option<proc_macro2::TokenStream>,
    kind: Kind,
    spawn: bool,
) -> proc_macro2::TokenStream {
    let mut items = vec![];
    let mut fields = vec![];

    let name = kind.ident();
    let priority = &ctxt.priority;

    let mut lt = None;
    if spawn {
        lt = Some(quote!('a));

        fields.push(quote!(
            /// Tasks that can be spawned from this context
            pub spawn: Spawn<'a>,
        ));

        if kind.is_idle() {
            items.push(quote!(
                /// Tasks that can be spawned from this context
                #[derive(Clone, Copy)]
                pub struct Spawn<'a> {
                    #[doc(hidden)]
                    pub #priority: &'a core::cell::Cell<u8>,
                }
            ));
        } else {
            items.push(quote!(
                /// Tasks that can be spawned from this context
                #[derive(Clone, Copy)]
                pub struct Spawn<'a> {
                    #[doc(hidden)]
                    pub #priority: &'a core::cell::Cell<u8>,
                }
            ));
        }
    }

    let mut root = None;
    if let Some(resources) = ctxt.resources.get(&kind) {
        lt = Some(quote!('a));

        root = Some(resources.decl.clone());

        let alias = &resources.alias;
        items.push(quote!(
            #[doc(inline)]
            pub use super::#alias as Resources;
        ));

        fields.push(quote!(
            /// Resources available in this context
            pub resources: Resources<'a>,
        ));
    };

    let doc = match kind {
        Kind::Idle => "Idle loop",
        Kind::Init => "Initialization function",
        Kind::Task(_) => "Software task",
    };

    quote!(
        #root

        #[doc = #doc]
        #cfg
        pub mod #name {
            /// Variables injected into this context by the `app` attribute
            pub struct Context<#lt> {
                #(#fields)*
            }

            #(#items)*
        }
    )
}

fn init(
    ctxt: &mut Context,
    cfg: &Option<proc_macro2::TokenStream>,
    init: &Init,
    app: &App,
    analysis: &Analysis,
) -> proc_macro2::TokenStream {
    let attrs = &init.attrs;
    let locals = mk_locals(&init.statics, true);
    let stmts = &init.stmts;
    let assigns = init
        .assigns
        .iter()
        .map(|assign| {
            if app
                .resources
                .get(&assign.left)
                .map(|r| r.expr.is_none())
                .unwrap_or(false)
            {
                let alias = &ctxt.statics[&assign.left];
                let expr = &assign.right;
                quote!(unsafe { #alias.set(#expr); })
            } else {
                let left = &assign.left;
                let right = &assign.right;
                quote!(#left = #right;)
            }
        })
        .collect::<Vec<_>>();

    let prelude = prelude(
        ctxt,
        cfg,
        Kind::Init,
        &init.args.resources,
        &init.args.spawn,
        app,
        255,
        analysis,
    );

    let module = module(ctxt, cfg, Kind::Init, !init.args.spawn.is_empty());

    let unsafety = &init.unsafety;
    let init = &ctxt.init;

    quote!(
        #module

        #(#attrs)*
        #cfg
        #unsafety fn #init() {
            #(#locals)*

            #prelude

            #(#stmts)*

            #(#assigns)*
        }
    )
}

fn idle(
    ctxt: &mut Context,
    cfg: &Option<proc_macro2::TokenStream>,
    idle: &Option<Idle>,
    app: &App,
    analysis: &Analysis,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if let Some(idle) = idle.as_ref() {
        let attrs = &idle.attrs;
        let locals = mk_locals(&idle.statics, true);
        let stmts = &idle.stmts;

        let prelude = prelude(
            ctxt,
            cfg,
            Kind::Idle,
            &idle.args.resources,
            &idle.args.spawn,
            app,
            0,
            analysis,
        );

        let module = module(ctxt, cfg, Kind::Idle, !idle.args.spawn.is_empty());

        let unsafety = &idle.unsafety;
        let idle = &ctxt.idle;

        (
            quote!(
                #module

                #(#attrs)*
                #cfg
                #unsafety fn #idle() -> ! {
                    #(#locals)*

                    #prelude

                    #(#stmts)*
                }
            ),
            quote!(#idle()),
        )
    } else {
        (
            quote!(),
            quote!(loop {
                // TODO WFI?
                // rtfm::export::wfi();
            }),
        )
    }
}

fn pre_init(ctxt: &Context, core: u8, app: &App, analysis: &Analysis) -> proc_macro2::TokenStream {
    let mut exprs = vec![];

    // Populate the `FreeQueue`s
    for (name, task) in &analysis.tasks {
        let task_ = &ctxt.tasks[name];

        if app.tasks[name].core == core {
            if task.local.capacity != 0 {
                let cap = task.local.capacity;
                let alias = &task_.local.as_ref().expect("unreachable").free_queue;

                exprs.push(quote!(
                    for i in 0..#cap {
                        #alias.enqueue_unchecked(i);
                    }
                ));
            }
        }

        if task.shared.capacity != 0 && core == 0 {
            // FIXME this initialization needs to be synchronized
            let cap = task.shared.capacity;
            let alias = &task_.shared.as_ref().expect("unreachable").free_queue;

            exprs.push(quote!(
                #alias.set(rtfm::export::FreeQueue::new());
                for i in 0..#cap {
                    #alias.get_mut().enqueue_unchecked(i);
                }
            ));
        }
    }

    // configure the ICC and ICD
    exprs.push(quote!(
        let mut icd = rtfm::export::ICD::take().unwrap();
        let mut icc = rtfm::export::ICC::take().unwrap();

        // disable interrupt routing and signaling during configuration
        icd.disable();
        icc.disable();

        // set priority mask to the lowest priority
        icc.ICCPMR.write(255);
    ));

    // Set SGIs priorities
    // Also, populate `#[shared]` `ReadyQueue`s
    for (priority, dispatcher) in &analysis.dispatchers[usize::from(core)] {
        let sgi = dispatcher.sgi;
        exprs.push(quote!(rtfm::export::ICD::set_priority(
            u16::from(#sgi),
            ((1 << #PRIORITY_BITS) - #priority) << (8 - #PRIORITY_BITS)
        );));

        let rq = &ctxt.dispatchers[usize::from(core)][&priority].ready_queue;
        if dispatcher.shared {
            exprs.push(quote!(
                #rq.set(rtfm::export::ReadyQueue::new());
            ));
        }
    }

    exprs.push(quote!(
        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        icd.enable();
    ));

    quote!(#(#exprs)*)
}

fn mk_resource(
    ctxt: &Context,
    cfg: &Option<proc_macro2::TokenStream>,
    struct_: &Ident,
    ty: proc_macro2::TokenStream,
    ceiling: u8,
    ptr: proc_macro2::TokenStream,
    module: Option<&mut Vec<proc_macro2::TokenStream>>,
) -> proc_macro2::TokenStream {
    let priority = &ctxt.priority;

    let mut items = vec![];

    let path = if let Some(module) = module {
        let doc = format!("`{}`", ty);
        module.push(quote!(
            #[doc = #doc]
            #cfg
            pub struct #struct_<'a> {
                #[doc(hidden)]
                pub #priority: &'a core::cell::Cell<u8>,
            }
        ));

        quote!(resources::#struct_)
    } else {
        items.push(quote!(
            #cfg
            struct #struct_<'a> {
                #priority: &'a core::cell::Cell<u8>,
            }
        ));

        quote!(#struct_)
    };

    items.push(quote!(
        #cfg
        impl<'a> rtfm::Mutex for #path<'a> {
            type T = #ty;

            #[inline]
            fn lock<R, F>(&mut self, f: F) -> R
            where
                F: FnOnce(&mut Self::T) -> R,
            {
                unsafe {
                    rtfm::export::claim(
                        #ptr,
                        &self.#priority,
                        #ceiling,
                        f,
                    )
                }
            }
        }
    ));

    quote!(#(#items)*)
}

// `once = true` means that these locals will be called from a function that will run *once*
fn mk_locals(locals: &HashMap<Ident, Static>, once: bool) -> proc_macro2::TokenStream {
    let lt = if once { Some(quote!('static)) } else { None };

    let locals = locals
        .iter()
        .map(|(name, static_)| {
            let attrs = &static_.attrs;
            let expr = &static_.expr;
            let ident = name;
            let ty = &static_.ty;

            quote!(
                #[allow(non_snake_case)]
                let #ident: &#lt mut #ty = {
                    #(#attrs)*
                    static mut #ident: #ty = #expr;

                    unsafe { &mut #ident }
                };
            )
        })
        .collect::<Vec<_>>();

    quote!(#(#locals)*)
}

/// The prelude injects `resources` and `spawn` (all values) into a function scope
fn prelude(
    ctxt: &mut Context,
    cfg: &Option<proc_macro2::TokenStream>,
    kind: Kind,
    resources: &Idents,
    spawn: &Idents,
    app: &App,
    logical_prio: u8,
    analysis: &Analysis,
) -> proc_macro2::TokenStream {
    let mut items = vec![];

    let lt = if kind.runs_once() {
        quote!('static)
    } else {
        quote!('a)
    };

    let module = kind.ident();

    let priority = &ctxt.priority;
    if !resources.is_empty() {
        let mut defs = vec![];
        let mut exprs = vec![];

        // NOTE This field is just to avoid unused type parameter errors around `'a`
        defs.push(quote!(#[allow(dead_code)] #priority: &'a core::cell::Cell<u8>));
        exprs.push(quote!(#priority));

        let mut may_call_lock = false;
        let mut needs_unsafe = false;
        for name in resources {
            let res = &app.resources[name];
            let initialized = res.expr.is_some();
            let mut_ = res.mutability;
            let ty = &res.ty;

            if kind.is_init() {
                let mut force_mut = false;
                if !analysis.ownerships.contains_key(name) {
                    // owned by Init
                    defs.push(quote!(pub #name: &'static #mut_ #ty));
                } else {
                    // owned by someone else
                    force_mut = true;
                    defs.push(quote!(pub #name: &'a mut #ty));
                }

                let alias = &ctxt.statics[name];
                // Resources assigned to init are always const initialized
                needs_unsafe = true;
                if force_mut {
                    exprs.push(quote!(#name: &mut #alias));
                } else {
                    exprs.push(quote!(#name: &#mut_ #alias));
                }
            } else {
                let ownership = &analysis.ownerships[name];
                let mut exclusive = false;

                if ownership.needs_lock(logical_prio) {
                    may_call_lock = true;
                    if mut_.is_none() {
                        defs.push(quote!(pub #name: &'a #ty));
                    } else {
                        // Generate a resource proxy
                        defs.push(quote!(pub #name: resources::#name<'a>));
                        exprs.push(quote!(#name: resources::#name { #priority }));
                        continue;
                    }
                } else {
                    if ownership.is_owned() || mut_.is_none() {
                        defs.push(quote!(pub #name: &#lt #mut_ #ty));
                    } else {
                        exclusive = true;
                        may_call_lock = true;
                        defs.push(quote!(pub #name: rtfm::Exclusive<#lt, #ty>));
                    }
                }

                let alias = &ctxt.statics[name];
                needs_unsafe = true;
                if initialized {
                    if exclusive {
                        exprs.push(quote!(#name: rtfm::Exclusive(&mut #alias)));
                    } else {
                        exprs.push(quote!(#name: &#mut_ #alias));
                    }
                } else {
                    let method = if mut_.is_some() {
                        quote!(get_mut)
                    } else {
                        quote!(get_ref)
                    };

                    if exclusive {
                        exprs.push(quote!(#name: rtfm::Exclusive(#alias.#method()) ));
                    } else {
                        exprs.push(quote!(#name: #alias.#method() ));
                    }
                }
            }
        }

        let alias = mk_ident(None);
        let unsafety = if needs_unsafe {
            Some(quote!(unsafe))
        } else {
            None
        };

        let doc = format!("`{}::Resources`", kind.ident().to_string());
        let decl = quote!(
            #[doc = #doc]
            #[allow(non_snake_case)]
            #cfg
            pub struct #alias<'a> { #(#defs,)* }
        );
        items.push(quote!(
            #[allow(unused_variables)]
            #[allow(unsafe_code)]
            #[allow(unused_mut)]
            let mut resources = #unsafety { #alias { #(#exprs,)* } };
        ));

        ctxt.resources
            .insert(kind.clone(), Resources { alias, decl });

        if may_call_lock {
            items.push(quote!(
                use rtfm::Mutex;
            ));
        }
    }

    if !spawn.is_empty() {
        if kind.is_idle() {
            items.push(quote!(
                #[allow(unused_variables)]
                let spawn = #module::Spawn { #priority };
            ));
        } else {
            let baseline_expr = match () {
                #[cfg(feature = "timer-queue")]
                () => {
                    let baseline = &ctxt.baseline;
                    quote!(#baseline)
                }
                #[cfg(not(feature = "timer-queue"))]
                () => quote!(),
            };
            items.push(quote!(
                #[allow(unused_variables)]
                let spawn = #module::Spawn { #priority, #baseline_expr };
            ));
        }
    }

    if items.is_empty() {
        quote!()
    } else {
        quote!(
            let ref #priority = core::cell::Cell::new(#logical_prio);

            #(#items)*
        )
    }
}

fn mk_ident(name: Option<&str>) -> Ident {
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let secs = elapsed.as_secs();
    let nanos = elapsed.subsec_nanos();

    let count = CALL_COUNT.fetch_add(1, Ordering::SeqCst) as u32;
    let mut seed: [u8; 16] = [0; 16];

    for (i, v) in seed.iter_mut().take(8).enumerate() {
        *v = ((secs >> (i * 8)) & 0xFF) as u8
    }

    for (i, v) in seed.iter_mut().skip(8).take(4).enumerate() {
        *v = ((nanos >> (i * 8)) & 0xFF) as u8
    }

    for (i, v) in seed.iter_mut().skip(12).enumerate() {
        *v = ((count >> (i * 8)) & 0xFF) as u8
    }

    let n;
    let mut s = if let Some(name) = name {
        n = 4;
        format!("{}_", name)
    } else {
        n = 16;
        String::new()
    };

    let mut rng = rand::rngs::SmallRng::from_seed(seed);
    for i in 0..n {
        if i == 0 || rng.gen() {
            s.push(('a' as u8 + rng.gen::<u8>() % 25) as char)
        } else {
            s.push(('0' as u8 + rng.gen::<u8>() % 10) as char)
        }
    }

    Ident::new(&s, Span::call_site())
}

fn mk_capacity_literal(capacity: u8) -> LitInt {
    LitInt::new(u64::from(capacity), IntSuffix::None, Span::call_site())
}

fn mk_typenum_capacity(capacity: u8, power_of_two: bool) -> proc_macro2::TokenStream {
    let capacity = if power_of_two {
        capacity
            .checked_next_power_of_two()
            .expect("capacity.next_power_of_two()")
    } else {
        capacity
    };

    let ident = Ident::new(&format!("U{}", capacity), Span::call_site());

    quote!(rtfm::export::consts::#ident)
}

fn mk_cfg(cores: u8, core: Option<u8>) -> Option<proc_macro2::TokenStream> {
    if cores == 1 {
        None
    } else {
        core.and_then(|core| {
            let core = core.to_string();
            Some(quote!(#[cfg(core = #core)]))
        })
    }
}

fn tuple_ty(inputs: &[ArgCaptured]) -> proc_macro2::TokenStream {
    if inputs.len() == 1 {
        let ty = &inputs[0].ty;
        quote!(#ty)
    } else {
        let tys = inputs.iter().map(|i| &i.ty).collect::<Vec<_>>();

        quote!((#(#tys,)*))
    }
}

fn tuple_pat(inputs: &[ArgCaptured]) -> proc_macro2::TokenStream {
    if inputs.len() == 1 {
        let pat = &inputs[0].pat;
        quote!(#pat)
    } else {
        let pats = inputs.iter().map(|i| &i.pat).collect::<Vec<_>>();

        quote!(#(#pats,)*)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Idle,
    Init,
    Task(Ident),
}

impl Kind {
    fn ident(&self) -> Ident {
        match self {
            Kind::Init => Ident::new("init", Span::call_site()),
            Kind::Idle => Ident::new("idle", Span::call_site()),
            Kind::Task(name) => name.clone(),
        }
    }

    fn is_idle(&self) -> bool {
        *self == Kind::Idle
    }

    fn is_init(&self) -> bool {
        *self == Kind::Init
    }

    fn runs_once(&self) -> bool {
        match *self {
            Kind::Init | Kind::Idle => true,
            _ => false,
        }
    }
}
