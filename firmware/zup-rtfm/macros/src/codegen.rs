use proc_macro::TokenStream;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use proc_macro2::Span;
use quote::quote;
use syn::{ArgCaptured, Attribute, Ident, IntSuffix, LitInt};

use crate::{
    analyze::{Analysis, Ownership},
    syntax::{App, Idents},
};

pub fn app(name: &Ident, app: &App, analysis: &Analysis) -> TokenStream {
    let (const_app_resources, mod_resources) = resources(app, analysis);

    // let (
    //     const_app_exceptions,
    //     exception_mods,
    //     exception_locals,
    //     exception_resources,
    //     user_exceptions,
    // ) = exceptions(app, analysis);

    let (
        const_app_interrupts,
        interrupt_mods,
        interrupt_locals,
        interrupt_resources,
        user_interrupts,
    ) = interrupts(app, analysis);

    let (const_app_tasks, task_mods, task_locals, task_resources, user_tasks) =
        tasks(app, analysis);

    let const_app_dispatchers = dispatchers(&app, analysis);

    let const_app_spawn = spawn(app, analysis);

    // let const_app_tq = timer_queue(app, analysis);

    // let const_app_schedule = schedule(app);

    let assertion_stmts = assertions(app, analysis);

    let (pre_init_stmts, const_app_pre_init) = pre_init(&app, analysis);

    let (
        const_app_init,
        mod_init,
        init_locals,
        init_resources,
        init_late_resources,
        user_init,
        call_init,
    ) = init(app, analysis);

    let post_init_stmts = post_init(&app, analysis);

    let (const_app_idle, mod_idle, idle_locals, idle_resources, user_idle, call_idle) =
        idle(app, analysis);

    quote!(
        #(#user_init)*

        #(#user_idle)*

        // #(#user_exceptions)*

        #(#user_interrupts)*

        #(#user_tasks)*

        #mod_resources

        #(#init_locals)*

        #(#init_resources)*

        #(#init_late_resources)*

        #(#mod_init)*

        #(#idle_locals)*

        #(#idle_resources)*

        #(#mod_idle)*

        // #(#exception_locals)*

        // #(#exception_resources)*

        // #(#exception_mods)*

        #(#interrupt_locals)*

        #(#interrupt_resources)*

        #(#interrupt_mods)*

        #(#task_locals)*

        #(#task_resources)*

        #(#task_mods)*

        /// Implementation details
        const #name: () = {
            #(#const_app_resources)*

            #(#const_app_pre_init)*

            #(#const_app_init)*

            #(#const_app_idle)*

            // #(#const_app_exceptions)*

            #(#const_app_interrupts)*

            #(#const_app_dispatchers)*

            #(#const_app_tasks)*

            #(#const_app_spawn)*

            // #(#const_app_tq)*

            // #(#const_app_schedule)*

            #[link_section = ".main"]
            #[no_mangle]
            unsafe fn main() -> ! {
                #(#assertion_stmts)*

                #(#pre_init_stmts)*

                #(#call_init)*

                #(#post_init_stmts)*

                #(#call_idle)*

                #[allow(unreachable_code)]
                loop {}
            }
        };
    )
    .into()
}

/* Main functions */
fn resources(
    app: &App,
    analysis: &Analysis,
) -> (
    // const_app
    Vec<proc_macro2::TokenStream>,
    // mod_resources
    proc_macro2::TokenStream,
) {
    let mut const_app = vec![];
    let mut mod_resources = vec![];

    for (name, core) in &analysis.locations {
        let res = &app.resources[name];

        let attrs = &res.attrs;
        let ty = &res.ty;
        let cfgs = &res.cfgs;

        let cfg_core = core.and_then(|core| app.cfg_core(core));
        let link_section = if core.is_some() {
            link_local(app, false)
        } else {
            Some(quote!(#[rtfm::export::shared]))
        };

        if let Some(expr) = res.expr.as_ref() {
            const_app.push(quote!(
                #(#attrs)*
                #(#cfgs)*
                #cfg_core
                #link_section
                static mut #name: #ty = #expr;
            ));
        } else {
            const_app.push(quote!(
                #(#attrs)*
                #(#cfgs)*
                #cfg_core
                #link_section
                static mut #name: rtfm::export::MaybeUninit<#ty> =
                    rtfm::export::MaybeUninit::uninit();
            ));
        }

        // generate a resource proxy when needed
        if res.mutability.is_some() {
            if let Some(Ownership::Shared { ceiling }) = analysis.ownerships.get(name) {
                let ptr = if res.expr.is_none() {
                    quote!(#name.as_mut_ptr())
                } else {
                    quote!(&mut #name)
                };

                mod_resources.push(quote!(
                    #cfg_core
                    pub struct #name<'a> {
                        priority: &'a Priority,
                    }

                    #cfg_core
                    impl<'a> #name<'a> {
                        #[inline(always)]
                        pub unsafe fn new(priority: &'a Priority) -> Self {
                            #name { priority }
                        }

                        #[inline(always)]
                        pub unsafe fn priority(&self) -> &Priority {
                            self.priority
                        }
                    }
                ));

                const_app.push(impl_mutex(
                    cfgs,
                    cfg_core,
                    true,
                    name,
                    quote!(#ty),
                    *ceiling,
                    ptr,
                ));
            }
        }
    }

    let mod_resources = if mod_resources.is_empty() {
        quote!()
    } else {
        quote!(
            mod resources {
                use rtfm::export::Priority;

                #(#mod_resources)*
            }
        )
    };

    (const_app, mod_resources)
}

fn interrupts(
    app: &App,
    analysis: &Analysis,
) -> (
    // const_app
    Vec<proc_macro2::TokenStream>,
    // interrupt_mods
    Vec<proc_macro2::TokenStream>,
    // interrupt_locals
    Vec<proc_macro2::TokenStream>,
    // interrupt_resources
    Vec<proc_macro2::TokenStream>,
    // user_exceptions
    Vec<proc_macro2::TokenStream>,
) {
    let mut const_app = vec![];
    let mut mods = vec![];
    let mut locals_structs = vec![];
    let mut resources_structs = vec![];
    let mut user_code = vec![];

    for (name, interrupt) in &app.interrupts {
        let priority = &interrupt.args.priority;
        let symbol = interrupt.args.binds(name);

        const_app.push(quote!(
            #[allow(non_snake_case)]
            #[no_mangle]
            unsafe fn #symbol() {
                const PRIORITY: u8 = #priority;

                // check that this interrupt exists
                let _ = rtfm::export::Interrupt::#symbol;

                rtfm::export::run(PRIORITY, || {
                    crate::#name(
                        #name::Locals::new(),
                        #name::Context::new(&rtfm::export::Priority::new(PRIORITY)),
                    )
                });
            }
        ));

        let mut needs_lt = false;
        if !interrupt.args.resources.is_empty() {
            let (item, constructor) = resources_struct(
                Kind::Interrupt(name.clone()),
                interrupt.args.priority,
                &mut needs_lt,
                app,
                analysis,
            );

            resources_structs.push(item);

            const_app.push(constructor);
        }

        mods.push(module(Kind::Interrupt(name.clone()), needs_lt, app));

        let attrs = &interrupt.attrs;
        let context = &interrupt.context;
        let (locals, lets) = locals(Kind::Interrupt(name.clone()), app);
        locals_structs.push(locals);
        let stmts = &interrupt.stmts;
        user_code.push(quote!(
            #(#attrs)*
            #[allow(non_snake_case)]
            fn #name(__locals: #name::Locals, #context: #name::Context) {
                use rtfm::Mutex as _;

                #(#lets;)*

                #(#stmts)*
            }
        ));
    }

    (
        const_app,
        mods,
        locals_structs,
        resources_structs,
        user_code,
    )
}

fn tasks(
    app: &App,
    analysis: &Analysis,
) -> (
    // const_app
    Vec<proc_macro2::TokenStream>,
    // task_mods
    Vec<proc_macro2::TokenStream>,
    // task_locals
    Vec<proc_macro2::TokenStream>,
    // task_resources
    Vec<proc_macro2::TokenStream>,
    // user_tasks
    Vec<proc_macro2::TokenStream>,
) {
    let mut const_app = vec![];
    let mut mods = vec![];
    let mut locals_structs = vec![];
    let mut resources_structs = vec![];
    let mut user_code = vec![];

    for (name, task) in &app.tasks {
        let inputs = &task.inputs;

        // create a free-queue + inputs buffer per sender
        if let Some(fq) = analysis.free_queues.get(name) {
            let receiver = task.args.core;
            let cap = task.args.capacity;
            let cap_lit = mk_capacity_literal(cap);
            let cap_ty = mk_typenum_capacity(cap, true);

            let (_, _, _, input_ty) = regroup_inputs(inputs);
            for (&sender, ceiling) in fq {
                let fq = mk_fq_ident(name, sender);
                let cfg_sender = app.cfg_core(sender);

                let (cfg_fq, mk_loc, fq_ty, expr) = if receiver == sender {
                    (
                        cfg_sender.clone(),
                        Box::new(|| link_local(app, false)) as Box<Fn() -> _>,
                        quote!(rtfm::export::SCFQ<#cap_ty>),
                        quote!(rtfm::export::Queue(unsafe {
                            rtfm::export::iQueue::u8_sc()
                        })),
                    )
                } else {
                    (
                        None,
                        Box::new(|| Some(quote!(#[rtfm::export::shared]))) as Box<Fn() -> _>,
                        quote!(rtfm::export::MCFQ<#cap_ty>),
                        quote!(rtfm::export::Queue(rtfm::export::iQueue::u8())),
                    )
                };

                let loc = mk_loc();
                const_app.push(quote!(
                    #cfg_fq
                    #loc
                    static mut #fq: #fq_ty = #expr;
                ));

                if let Some(ceiling) = ceiling {
                    const_app.push(quote!(
                        #cfg_sender
                        struct #fq<'a> {
                            priority: &'a rtfm::export::Priority,
                        }
                    ));

                    const_app.push(impl_mutex(
                        &[],
                        cfg_sender.clone(),
                        false,
                        &fq,
                        fq_ty,
                        *ceiling,
                        quote!(&mut #fq),
                    ));
                }

                let elems = (0..cap)
                    .map(|_| quote!(rtfm::export::MaybeUninit::uninit()))
                    .collect::<Vec<_>>();

                let loc = mk_loc();
                let inputs = mk_inputs_ident(name, sender);
                const_app.push(quote!(
                    #cfg_fq
                    #loc
                    static mut #inputs: [rtfm::export::MaybeUninit<#input_ty>; #cap_lit] =
                        [#(#elems,)*];
                ));
            }
        }

        let mut needs_lt = false;
        if !task.args.resources.is_empty() {
            let (item, constructor) = resources_struct(
                Kind::Task(name.clone()),
                task.args.priority,
                &mut needs_lt,
                app,
                analysis,
            );

            resources_structs.push(item);

            const_app.push(constructor);
        }

        mods.push(module(Kind::Task(name.clone()), needs_lt, app));

        let attrs = &task.attrs;
        let cfg_receiver = app.cfg_core(task.args.core);
        let context = &task.context;
        let inputs = &task.inputs;
        let (locals_struct, lets) = locals(Kind::Task(name.clone()), app);
        let stmts = &task.stmts;
        user_code.push(quote!(
            #(#attrs)*
            #[allow(non_snake_case)]
            #cfg_receiver
            fn #name(__locals: #name::Locals, #context: #name::Context #(,#inputs)*) {
                use rtfm::Mutex as _;

                #(#lets;)*

                #(#stmts)*
            }
        ));

        locals_structs.push(locals_struct);
    }

    (
        const_app,
        mods,
        locals_structs,
        resources_structs,
        user_code,
    )
}

fn dispatchers(app: &App, analysis: &Analysis) -> Vec<proc_macro2::TokenStream> {
    let mut items = vec![];

    for (dispatchers, receiver) in analysis.dispatchers.iter().zip(0..) {
        for (&priority, routes) in dispatchers {
            let mut drains = vec![];
            for (&sender, route) in routes {
                let rq = mk_rq_ident(receiver, sender, priority);
                let cap = route.capacity(app);
                let cap_ty = mk_typenum_capacity(cap, true);
                let t = mk_t_ident(receiver, sender, priority);

                let variants = route
                    .tasks
                    .iter()
                    .map(|name| {
                        let cfgs = &app.tasks[name].cfgs;

                        quote!(
                            #(#cfgs)*
                            #name
                        )
                    })
                    .collect::<Vec<_>>();

                let cfg_sender = app.cfg_core(sender);
                let (cfg_rq, mk_loc, rq_ty, expr) = if receiver == sender {
                    (
                        cfg_sender.clone(),
                        Box::new(|| link_local(app, false)) as Box<Fn() -> _>,
                        quote!(rtfm::export::SCRQ<#t, #cap_ty>),
                        quote!(rtfm::export::Queue(unsafe {
                            rtfm::export::iQueue::u8_sc()
                        })),
                    )
                } else {
                    (
                        None,
                        Box::new(|| Some(quote!(#[rtfm::export::shared]))) as Box<Fn() -> _>,
                        quote!(rtfm::export::MCRQ<#t, #cap_ty>),
                        quote!(rtfm::export::Queue(rtfm::export::iQueue::u8())),
                    )
                };

                items.push(quote!(
                    #[allow(non_camel_case_types)]
                    #cfg_rq
                    enum #t {
                        #(#variants,)*
                    }
                ));

                let loc = mk_loc();
                items.push(quote!(
                    #cfg_rq
                    #loc
                    static mut #rq: #rq_ty = #expr;
                ));

                if let Some(ceiling) = route.ceiling {
                    items.push(quote!(
                        #cfg_sender
                        struct #rq<'a> {
                            priority: &'a rtfm::export::Priority,
                        }
                    ));

                    items.push(impl_mutex(
                        &[],
                        cfg_sender.clone(),
                        false,
                        &rq,
                        rq_ty,
                        ceiling,
                        quote!(&mut #rq),
                    ));
                }

                let arms = route
                    .tasks
                    .iter()
                    .map(|name| {
                        let task = &app.tasks[name];
                        let cfgs = &task.cfgs;
                        let fq = mk_fq_ident(name, sender);
                        let inputs = mk_inputs_ident(name, sender);
                        let (_, tupled, pats, _) = regroup_inputs(&task.inputs);

                        let input = quote!(
                            #inputs.get_unchecked(usize::from(index)).read()
                        );

                        quote!(
                            #(#cfgs)*
                            #t::#name => {
                                let #tupled = #input;
                                #fq.split().0.enqueue_unchecked(index);
                                let priority = &rtfm::export::Priority::new(PRIORITY);
                                #name(
                                    #name::Locals::new(),
                                    #name::Context::new(priority)
                                    #(,#pats)*
                                )
                            }
                        )
                    })
                    .collect::<Vec<_>>();

                drains.push(quote!(
                    while let Some((task, index)) = #rq.split().1.dequeue() {
                        match task {
                            #(#arms)*
                        }
                    }
                ))
            }

            let cfg_receiver = app.cfg_core(receiver);
            let sg = mk_sg_ident(analysis.sgis[usize::from(receiver)][&priority]);
            items.push(quote!(
                #[no_mangle]
                #cfg_receiver
                unsafe fn #sg() {
                    /// The priority of this interrupt handler
                    const PRIORITY: u8 = #priority;

                    // check that the interrupt exists
                    let _ = rtfm::export::Interrupt::#sg;

                    rtfm::export::run(PRIORITY, || {
                        #(#drains)*
                    });
                }
            ));
        }
    }

    items
}

fn spawn(app: &App, analysis: &Analysis) -> Vec<proc_macro2::TokenStream> {
    let mut items = vec![];

    // TODO `spawn_` optimization
    for (sender, spawner, spawnees) in app.spawn_callers() {
        if spawnees.is_empty() {
            continue;
        }

        let spawner_is_init = spawner == "init";

        let mut methods = vec![];
        for name in spawnees {
            let spawnee = &app.tasks[name];
            let receiver = spawnee.args.core;
            let priority = spawnee.args.priority;
            let cfgs = &spawnee.cfgs;
            let (args, tupled, _, ty) = regroup_inputs(&spawnee.inputs);

            let fq = mk_fq_ident(name, sender);
            let inputs = mk_inputs_ident(name, sender);
            let t = mk_t_ident(receiver, sender, priority);
            let rq = mk_rq_ident(receiver, sender, priority);
            let sg = analysis.sgis[usize::from(receiver)][&priority];

            let (let_priority, dequeue, enqueue) = if spawner_is_init {
                (
                    None,
                    quote!(#fq.dequeue()),
                    quote!(#rq.enqueue_unchecked((#t::#name, index));),
                )
            } else {
                (
                    Some(quote!(let priority = self.priority();)),
                    quote!((#fq { priority }).lock(|fq| fq.split().1.dequeue())),
                    quote!((#rq { priority }).lock(|rq| {
                        rq.split().0.enqueue_unchecked((#t::#name, index))
                    });),
                )
            };

            let target = if sender == receiver {
                quote!(rtfm::export::Target::Loopback)
            } else {
                quote!(rtfm::export::Target::Unicast(#receiver))
            };

            methods.push(quote!(
                #(#cfgs)*
                fn #name(&self #(,#args)*) -> Result<(), #ty> {
                    unsafe {
                        use rtfm::Mutex as _;

                        #let_priority
                        let input = #tupled;
                        if let Some(index) = #dequeue {
                            #inputs.get_unchecked_mut(usize::from(index)).write(input);

                            #enqueue

                            rtfm::export::ICD::icdsgir(#target, #sg);

                            Ok(())
                        } else {
                            Err(input)
                        }
                    }
                }
            ));
        }

        let cfg_core = app.cfg_core(sender);
        let lt = if spawner_is_init {
            None
        } else {
            Some(quote!('a))
        };
        items.push(quote!(
            #cfg_core
            impl<#lt> #spawner::Spawn<#lt> {
                #(#methods)*
            }
        ))
    }

    items
}

fn assertions(app: &App, analysis: &Analysis) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = vec![];

    for ty in &analysis.assert_sync {
        stmts.push(quote!(rtfm::export::assert_sync::<#ty>();));
    }

    for task in &analysis.tasks_assert_send {
        let (_, _, _, ty) = regroup_inputs(&app.tasks[task].inputs);
        stmts.push(quote!(rtfm::export::assert_send::<#ty>();));
    }

    for task in &analysis.tasks_assert_local_send {
        let (_, _, _, ty) = regroup_inputs(&app.tasks[task].inputs);
        let assert = if app.cores == 1 {
            quote!(assert_send)
        } else {
            quote!(assert_local_send)
        };
        stmts.push(quote!(rtfm::export::#assert::<#ty>();));
    }

    for ty in &analysis.resources_assert_send {
        stmts.push(quote!(rtfm::export::assert_send::<#ty>();));
    }

    for ty in &analysis.resources_assert_local_send {
        let assert = if app.cores == 1 {
            quote!(assert_send)
        } else {
            quote!(assert_local_send)
        };
        stmts.push(quote!(rtfm::export::#assert::<#ty>();));
    }

    stmts
}

fn pre_init(
    app: &App,
    analysis: &Analysis,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut const_app = vec![];
    let mut stmts = vec![];

    for (task, fq) in &analysis.free_queues {
        let cap = app.tasks[task].args.capacity;
        for &sender in fq.keys() {
            let cfg_sender = app.cfg_core(sender);
            let fq = mk_fq_ident(task, sender);

            stmts.push(quote!(
                #cfg_sender
                for i in 0..#cap {
                    #fq.enqueue_unchecked(i);
                }
            ));
        }
    }

    if app.cores == 1 {
        stmts.push(quote!(
            rtfm::export::setup_counter();
        ));
    } else {
        stmts.push(quote!(
            #[rtfm::export::shared]
            static __RV__: core::sync::atomic::AtomicBool =
                core::sync::atomic::AtomicBool::new(false);
        ));

        stmts.push(quote!(if __RV__
            .compare_exchange_weak(
                false,
                true,
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
        {
            rtfm::export::setup_counter();
        }));
    }

    stmts.push(quote!(
        rtfm::export::ICC::set_iccpmr(!0);
        rtfm::export::ICC::set_iccicr((1 << 1) | (1 << 0));
        rtfm::export::clear_sgis();
        rtfm::export::ICD::enable();
    ));

    if app.cores != 1 {
        for &receiver in analysis.pre_rendezvous.keys() {
            let rv = mk_pre_rv_ident(receiver);

            const_app.push(quote!(
                #[rtfm::export::shared]
                static #rv: core::sync::atomic::AtomicBool =
                    core::sync::atomic::AtomicBool::new(false);
            ));

            let cfg_receiver = app.cfg_core(receiver);
            stmts.push(quote!(
                #cfg_receiver
                #rv.store(true, core::sync::atomic::Ordering::Release);
            ));
        }

        for (&receiver, senders) in &analysis.pre_rendezvous {
            let rv = mk_pre_rv_ident(receiver);

            for &sender in senders {
                let cfg_sender = app.cfg_core(sender);
                stmts.push(quote!(
                    #cfg_sender
                    while !#rv.load(core::sync::atomic::Ordering::Acquire) {}
                ));
            }
        }
    }

    for (sgis, core) in analysis.sgis.iter().zip(0..) {
        let cfg_core = app.cfg_core(core);

        for (priority, sgi) in sgis.iter() {
            stmts.push(quote!(
                #cfg_core
                rtfm::export::ICD::set_priority(
                    u16::from(#sgi),
                    rtfm::export::logical2hw(#priority + 1),
                );
            ));
        }
    }

    (stmts, const_app)
}

fn init(
    app: &App,
    analysis: &Analysis,
) -> (
    // const_app
    Vec<proc_macro2::TokenStream>,
    // mod_init
    Vec<proc_macro2::TokenStream>,
    // init_locals
    Vec<proc_macro2::TokenStream>,
    // init_resources
    Vec<proc_macro2::TokenStream>,
    // init_late_resources
    Vec<proc_macro2::TokenStream>,
    // user_init
    Vec<proc_macro2::TokenStream>,
    // call_init
    Vec<proc_macro2::TokenStream>,
) {
    let mut const_app = vec![];
    let mut mod_init = vec![];
    let mut init_locals = vec![];
    let mut init_resources = vec![];
    let mut init_late_resources = vec![];
    let mut user_init = vec![];
    let mut call_init = vec![];

    for (core, init) in app
        .mains
        .iter()
        .zip(0..)
        .filter_map(|(main, core)| main.init.as_ref().map(|init| (core, init)))
    {
        let mut needs_lt = false;

        if !init.args.resources.is_empty() {
            let (item, constructor) =
                resources_struct(Kind::Init(core), 0, &mut needs_lt, app, analysis);

            init_resources.push(item);
            const_app.push(constructor);
        }

        let cfg_core = app.cfg_core(core);
        call_init.push(quote!(
            #cfg_core
            let late = init(init::Locals::new(), init::Context::new());
        ));

        let ret = if let Some(late) = analysis.late_resources.get(&core) {
            let late_fields = late
                .iter()
                .map(|name| {
                    let ty = &app.resources[name].ty;

                    quote!(pub #name: #ty)
                })
                .collect::<Vec<_>>();

            init_late_resources.push(quote!(
                /// Resources initialized at runtime
                #[allow(non_snake_case)]
                #cfg_core
                pub struct initLateResources {
                    #(#late_fields),*
                }
            ));

            Some(quote!(-> init::LateResources))
        } else {
            None
        };

        let (locals_struct, lets) = locals(Kind::Init(core), &app);
        init_locals.push(locals_struct);

        let context = &init.context;
        let attrs = &init.attrs;
        let stmts = &init.stmts;
        user_init.push(quote!(
            #(#attrs)*
            #[allow(non_snake_case)]
            #cfg_core
            fn init(__locals: init::Locals, #context: init::Context) #ret {
                #(#lets;)*

                #(#stmts)*
            }
        ));

        mod_init.push(module(Kind::Init(core), needs_lt, app));
    }

    (
        const_app,
        mod_init,
        init_locals,
        init_resources,
        init_late_resources,
        user_init,
        call_init,
    )
}

fn post_init(app: &App, analysis: &Analysis) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = vec![];

    // initialize late resources
    for (core, late) in &analysis.late_resources {
        let cfg_core = app.cfg_core(*core);
        for name in late {
            stmts.push(quote!(
                #cfg_core
                #name.write(late.#name);
            ));
        }
    }

    let initializers = analysis
        .post_rendezvous
        .iter()
        .flat_map(|(_, initializers)| initializers)
        .collect::<BTreeSet<_>>();
    for &initializer in initializers {
        let rv = mk_post_rv_ident(initializer);

        let cfg_initializer = app.cfg_core(initializer);
        stmts.push(quote!(
            #[rtfm::export::shared]
            static #rv: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

            #cfg_initializer
            #rv.store(true, core::sync::atomic::Ordering::Release);
        ));
    }

    for (&initializer, users) in &analysis.post_rendezvous {
        let rv = mk_post_rv_ident(initializer);

        for &user in users {
            let cfg_user = app.cfg_core(user);
            stmts.push(quote!(
                #cfg_user
                while !#rv.load(core::sync::atomic::Ordering::Acquire) {}
            ));
        }
    }

    stmts.push(quote!(
        rtfm::export::enable_irq();
    ));

    stmts
}

fn idle(
    app: &App,
    analysis: &Analysis,
) -> (
    // const_app_idle
    Vec<proc_macro2::TokenStream>,
    // mod_idle
    Vec<proc_macro2::TokenStream>,
    // idle_locals
    Vec<proc_macro2::TokenStream>,
    // idle_resources
    Vec<proc_macro2::TokenStream>,
    // user_idle
    Vec<proc_macro2::TokenStream>,
    // call_idle
    Vec<proc_macro2::TokenStream>,
) {
    let mut const_app = vec![];
    let mut mod_idle = vec![];
    let mut idle_locals = vec![];
    let mut idle_resources = vec![];
    let mut user_idle = vec![];
    let mut call_idle = vec![];

    for (core, idle) in app.mains.iter().zip(0..).filter_map(|(main, core)| {
        if let Some(idle) = main.idle.as_ref() {
            Some((core, idle))
        } else {
            None
        }
    }) {
        let mut needs_lt = false;

        if !idle.args.resources.is_empty() {
            let (item, constructor) =
                resources_struct(Kind::Idle(core), 0, &mut needs_lt, app, analysis);

            idle_resources.push(item);
            const_app.push(constructor);
        }

        let cfg_core = app.cfg_core(core);
        call_idle.push(quote!(
            #cfg_core
            idle(
                idle::Locals::new(),
                idle::Context::new(&rtfm::export::Priority::new(0))
            );
        ));

        let attrs = &idle.attrs;
        let context = &idle.context;
        let (locals, lets) = locals(Kind::Idle(core), app);
        idle_locals.push(locals);
        let stmts = &idle.stmts;
        user_idle.push(quote!(
            #(#attrs)*
            #[allow(non_snake_case)]
            #cfg_core
            fn idle(__locals: idle::Locals, #context: idle::Context) -> ! {
                use rtfm::Mutex as _;

                #(#lets;)*

                #(#stmts)*
            }
        ));

        mod_idle.push(module(Kind::Idle(core), needs_lt, app));
    }

    (
        const_app,
        mod_idle,
        idle_locals,
        idle_resources,
        user_idle,
        call_idle,
    )
}

/* Support code */
fn resources_struct(
    kind: Kind,
    priority: u8,
    needs_lt: &mut bool,
    app: &App,
    analysis: &Analysis,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut lt = None;

    let (core, resources) = match &kind {
        Kind::Init(core) => (
            *core,
            &app.mains[usize::from(*core)]
                .init
                .as_ref()
                .expect("UNREACHABLE")
                .args
                .resources,
        ),

        Kind::Idle(core) => (
            *core,
            &app.mains[usize::from(*core)]
                .idle
                .as_ref()
                .expect("UNREACHABLE")
                .args
                .resources,
        ),

        Kind::Interrupt(name) => {
            let interrupt = &app.interrupts[name];
            (interrupt.args.core, &interrupt.args.resources)
        }

        Kind::Task(name) => {
            let task = &app.tasks[name];
            (task.args.core, &task.args.resources)
        }
    };

    let mut fields = vec![];
    let mut values = vec![];
    let mut has_cfgs = false;

    for name in resources {
        let res = &app.resources[name];

        let cfgs = &res.cfgs;
        let mut_ = res.mutability;
        let ty = &res.ty;

        has_cfgs |= true;
        if kind.is_init() {
            if !analysis.ownerships.contains_key(name) {
                // owned by `init`
                if app.cores != 1 {
                    if mut_.is_some() {
                        fields.push(quote!(
                            #(#cfgs)*
                            pub #name: rtfm::LocalMut<#ty>
                        ));

                        values.push(quote!(
                            #(#cfgs)*
                            #name: rtfm::LocalMut::pin(&mut #name)
                        ));
                    } else {
                        fields.push(quote!(
                            #(#cfgs)*
                            pub #name: rtfm::LocalRef<#ty>
                        ));

                        values.push(quote!(
                            #(#cfgs)*
                            #name: rtfm::LocalRef::pin(&#name)
                        ));
                    }
                } else {
                    fields.push(quote!(
                        #(#cfgs)*
                        pub #name: &'static #mut_ #ty
                    ));

                    values.push(quote!(
                        #(#cfgs)*
                        #name: &#mut_ #name
                    ));
                }
            } else {
                // owned by someone else
                lt = Some(quote!('a));

                fields.push(quote!(
                    #(#cfgs)*
                    pub #name: &'a mut #ty
                ));

                values.push(quote!(
                    #(#cfgs)*
                    #name: &mut #name
                ));
            }
        } else {
            let ownership = &analysis.ownerships[name];

            if ownership.needs_lock(priority) {
                if mut_.is_none() {
                    lt = Some(quote!('a));

                    fields.push(quote!(
                        #(#cfgs)*
                        pub #name: &'a #ty
                    ));
                } else {
                    // resource proxy
                    lt = Some(quote!('a));

                    fields.push(quote!(
                        #(#cfgs)*
                        pub #name: resources::#name<'a>
                    ));

                    values.push(quote!(
                        #(#cfgs)*
                        #name: resources::#name::new(priority)
                    ));

                    continue;
                }
            } else {
                if kind.runs_once() && app.cores != 1 && analysis.locations[name].is_some() {
                    if mut_.is_some() {
                        fields.push(quote!(
                            #(#cfgs)*
                            pub #name: rtfm::LocalMut<#ty>
                        ));

                        if res.expr.is_none() {
                            values.push(quote!(
                                #(#cfgs)*
                                #name: rtfm::LocalMut::pin(&mut *(#name.as_mut_ptr()))
                            ));
                        } else {
                            values.push(quote!(
                                #(#cfgs)*
                                #name: rtfm::LocalMut::pin(&mut #name)
                            ));
                        }
                    } else {
                        fields.push(quote!(
                            #(#cfgs)*
                            pub #name: rtfm::LocalRef<#ty>
                        ));

                        if res.expr.is_none() {
                            values.push(quote!(
                                #(#cfgs)*
                                #name: rtfm::LocalRef::pin(&*(#name.as_ptr()))
                            ));
                        } else {
                            values.push(quote!(
                                #(#cfgs)*
                                #name: rtfm::LocalRef::pin(&#name)
                            ));
                        }
                    }

                    continue;
                }

                let lt = if kind.runs_once() {
                    quote!('static)
                } else {
                    lt = Some(quote!('a));
                    quote!('a)
                };

                if ownership.is_owned() || mut_.is_none() {
                    fields.push(quote!(
                        #(#cfgs)*
                        pub #name: &#lt #mut_ #ty
                    ));
                } else {
                    fields.push(quote!(
                        #(#cfgs)*
                        pub #name: &#lt mut #ty
                    ));
                }
            }

            let is_late = res.expr.is_none();
            if is_late {
                let expr = if mut_.is_some() {
                    quote!(&mut *#name.as_mut_ptr())
                } else {
                    quote!(&*#name.as_ptr())
                };

                values.push(quote!(
                    #(#cfgs)*
                    #name: #expr
                ));
            } else {
                values.push(quote!(
                    #(#cfgs)*
                    #name: &#mut_ #name
                ));
            }
        }
    }

    if lt.is_some() {
        *needs_lt = true;

        if has_cfgs {
            // the struct could end up empty due to `cfg` leading to an error due to `'a` being
            // unused so insert a dummy field to "use" the lifetime parameter
            fields.push(quote!(
                #[doc(hidden)]
                pub __marker__: core::marker::PhantomData<&'a ()>
            ));

            values.push(quote!(__marker__: core::marker::PhantomData))
        }
    }

    let cfg_core = app.cfg_core(core);
    let ident = kind.resources_ident();
    let doc = format!("Resources {} has access to", kind.ident());
    let item = quote!(
        #[allow(non_snake_case)]
        #[doc = #doc]
        #cfg_core
        pub struct #ident<#lt> {
            #(#fields,)*
        }
    );
    let arg = if kind.is_init() {
        None
    } else {
        Some(quote!(priority: &#lt rtfm::export::Priority))
    };
    let constructor = quote!(
        #cfg_core
        impl<#lt> #ident<#lt> {
            #[inline(always)]
            unsafe fn new(#arg) -> Self {
                #ident {
                    #(#values,)*
                }
            }
        }
    );

    (item, constructor)
}

/// Creates a `Mutex` implementation
fn impl_mutex(
    cfgs: &[Attribute],
    cfg_core: Option<proc_macro2::TokenStream>,
    resources_prefix: bool,
    name: &Ident,
    ty: proc_macro2::TokenStream,
    ceiling: u8,
    ptr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let path = if resources_prefix {
        quote!(resources::#name)
    } else {
        quote!(#name)
    };

    let priority = if resources_prefix {
        quote!(self.priority())
    } else {
        quote!(self.priority)
    };

    quote!(
        #(#cfgs)*
        #cfg_core
        impl<'a> rtfm::Mutex for #path<'a> {
            type T = #ty;

            #[inline(always)]
            fn lock<R>(&mut self, f: impl FnOnce(&mut #ty) -> R) -> R {
                /// Priority ceiling
                const CEILING: u8 = #ceiling;

                unsafe {
                    rtfm::export::lock(
                        #ptr,
                        #priority,
                        CEILING,
                        f,
                    )
                }
            }
        }
    )
}

fn locals(
    kind: Kind,
    app: &App,
) -> (
    // locals
    proc_macro2::TokenStream,
    // lets
    Vec<proc_macro2::TokenStream>,
) {
    let runs_once = kind.runs_once();
    let ident = kind.locals_ident();
    let (core, statics) = match kind {
        Kind::Init(core) => (
            core,
            &app.mains[usize::from(core)]
                .init
                .as_ref()
                .expect("UNREACHABLE")
                .statics,
        ),

        Kind::Idle(core) => (
            core,
            &app.mains[usize::from(core)]
                .idle
                .as_ref()
                .expect("UNREACHABLE")
                .statics,
        ),

        Kind::Interrupt(name) => {
            let interrupt = &app.interrupts[&name];

            (interrupt.args.core, &interrupt.statics)
        }

        Kind::Task(name) => {
            let task = &app.tasks[&name];

            (task.args.core, &task.statics)
        }
    };

    let mut lt = None;
    let mut fields = vec![];
    let mut lets = vec![];
    let mut items = vec![];
    let mut values = vec![];

    let mut has_cfgs = false;
    for (name, static_) in statics {
        let attrs = &static_.attrs;
        let cfgs = &static_.cfgs;
        let expr = &static_.expr;
        let global = static_.global;
        let ty = &static_.ty;

        let lt = if runs_once {
            if app.cores != 1 && !global {
                fields.push(quote!(
                    #(#cfgs)*
                    #name: rtfm::LocalMut<#ty>
                ));

                values.push(quote!(
                    #(#cfgs)*
                    #name: rtfm::LocalMut::pin(&mut #name)
                ));

                None
            } else {
                Some(quote!('static))
            }
        } else {
            lt = Some(quote!('a));
            lt.clone()
        };

        if !cfgs.is_empty() {
            has_cfgs = true;
        }

        let link_local = link_local(app, global);
        items.push(quote!(
            #(#attrs)*
            #(#cfgs)*
            #link_local
            static mut #name: #ty = #expr
        ));

        if let Some(lt) = lt {
            fields.push(quote!(
                #(#cfgs)*
                #name: &#lt mut #ty
            ));

            values.push(quote!(
                #(#cfgs)*
                #name: &mut #name
            ));
        }

        lets.push(quote!(
            #(#cfgs)*
            let #name = __locals.#name
        ));
    }

    if lt.is_some() && has_cfgs {
        fields.push(quote!(__marker__: core::marker::PhantomData<&'a mut ()>));
        values.push(quote!(__marker__: core::marker::PhantomData));
    }

    let cfg_core = app.cfg_core(core);
    let locals = quote!(
        #cfg_core
        #[allow(non_snake_case)]
        #[doc(hidden)]
        pub struct #ident<#lt> {
            #(#fields),*
        }

        #cfg_core
        impl<#lt> #ident<#lt> {
            #[inline(always)]
            unsafe fn new() -> Self {
                #(#items;)*

                #ident {
                    #(#values),*
                }
            }
        }
    );

    (locals, lets)
}

fn module(kind: Kind, resources_lt: bool, app: &App) -> proc_macro2::TokenStream {
    let mut items = vec![];
    let mut fields = vec![];
    let mut values = vec![];

    let name = kind.ident();
    let mut lt = None;

    let ident = kind.locals_ident();
    items.push(quote!(
        #[doc(inline)]
        pub use super::#ident as Locals;
    ));

    if !kind.resources(app).is_empty() {
        let ident = kind.resources_ident();
        let lt = if resources_lt {
            lt = Some(quote!('a));
            Some(quote!('a))
        } else {
            None
        };

        items.push(quote!(
            #[doc(inline)]
            pub use super::#ident as Resources;
        ));

        fields.push(quote!(
            /// Resources this task has access to
            pub resources: Resources<#lt>
        ));

        let priority = if kind.is_init() {
            None
        } else {
            Some(quote!(priority))
        };
        values.push(quote!(resources: Resources::new(#priority)));
    }

    if !kind.spawn(app).is_empty() {
        let doc = "Tasks that can be `spawn`-ed from this context";
        if kind.is_init() {
            fields.push(quote!(
                #[doc = #doc]
                pub spawn: Spawn
            ));

            items.push(quote!(
                #[doc = #doc]
                #[derive(Clone, Copy)]
                pub struct Spawn {
                    _not_send: core::marker::PhantomData<*mut ()>,
                }
            ));

            values.push(quote!(spawn: Spawn { _not_send: core::marker::PhantomData }));
        } else {
            lt = Some(quote!('a));

            fields.push(quote!(
                #[doc = #doc]
                pub spawn: Spawn<'a>
            ));

            if kind.is_idle() {
                items.push(quote!(
                    #[doc = #doc]
                    #[derive(Clone, Copy)]
                    pub struct Spawn<'a> {
                        priority: &'a rtfm::export::Priority,
                    }
                ));

                values.push(quote!(spawn: Spawn { priority }));
            } else {
                items.push(quote!(
                    /// Tasks that can be spawned from this context
                    #[derive(Clone, Copy)]
                    pub struct Spawn<'a> {
                        priority: &'a rtfm::export::Priority,
                    }
                ));

                values.push(quote!(
                    spawn: Spawn { priority }
                ));
            }

            items.push(quote!(
                impl<'a> Spawn<'a> {
                    #[doc(hidden)]
                    #[inline(always)]
                    pub unsafe fn priority(&self) -> &rtfm::export::Priority {
                        self.priority
                    }
                }
            ));
        }
    }

    if kind.returns_late_resources(app) {
        items.push(quote!(
            #[doc(inline)]
            pub use super::initLateResources as LateResources;
        ));
    }

    let priority = if kind.is_init() {
        None
    } else {
        Some(quote!(priority: &#lt rtfm::export::Priority))
    };

    items.push(quote!(
        /// Execution context
        pub struct Context<#lt> {
            #(#fields,)*
        }

        impl<#lt> Context<#lt> {
            #[inline(always)]
            pub unsafe fn new(#priority) -> Self {
                Context {
                    #(#values,)*
                }
            }
        }
    ));

    let doc = kind.doc();
    let cfg_core = kind.cfg_core(app);
    if !items.is_empty() {
        quote!(
            #[allow(non_snake_case)]
            #[doc = #doc]
            #cfg_core
            pub mod #name {
                #(#items)*
            }
        )
    } else {
        quote!()
    }
}

fn link_local(app: &App, global: bool) -> Option<proc_macro2::TokenStream> {
    if app.cores == 1 || global {
        None
    } else {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let section = format!(".local.{}", COUNT.fetch_add(1, Ordering::Relaxed));
        Some(quote!(
            #[link_section = #section]
        ))
    }
}

/// `u8` -> (unsuffixed) `LitInt`
fn mk_capacity_literal(capacity: u8) -> LitInt {
    LitInt::new(u64::from(capacity), IntSuffix::None, Span::call_site())
}

/// e.g. `4u8` -> `U4`
fn mk_typenum_capacity(capacity: u8, power_of_two: bool) -> proc_macro2::TokenStream {
    let capacity = if power_of_two {
        capacity.checked_next_power_of_two().expect("UNREACHABLE")
    } else {
        capacity
    };

    let ident = Ident::new(&format!("U{}", capacity), Span::call_site());

    quote!(rtfm::export::consts::#ident)
}

/// e.g. `foo_S2_INPUTS`
fn mk_inputs_ident(task: &Ident, sender: u8) -> Ident {
    Ident::new(&format!("{}_S{}_INPUTS", task, sender), Span::call_site())
}

/// e.g. `foo_S1_FQ`
fn mk_fq_ident(task: &Ident, sender: u8) -> Ident {
    Ident::new(&format!("{}_S{}_FQ", task, sender), Span::call_site())
}

/// e.g. `R0_S1_RQ3`
fn mk_rq_ident(receiver: u8, sender: u8, priority: u8) -> Ident {
    Ident::new(
        &format!("R{}_S{}_RQ{}", receiver, sender, priority),
        Span::call_site(),
    )
}

/// e.g. `R0_S1_T3`
fn mk_t_ident(receiver: u8, sender: u8, priority: u8) -> Ident {
    Ident::new(
        &format!("R{}_S{}_T{}", receiver, sender, priority),
        Span::call_site(),
    )
}

/// e.g. `SG0`
fn mk_sg_ident(i: u8) -> Ident {
    Ident::new(&format!("SG{}", i), Span::call_site())
}

fn mk_pre_rv_ident(core: u8) -> Ident {
    Ident::new(&format!("__PRE_RV{}__", core), Span::call_site())
}

fn mk_post_rv_ident(core: u8) -> Ident {
    Ident::new(&format!("__POST_RV{}__", core), Span::call_site())
}

fn regroup_inputs(
    inputs: &[ArgCaptured],
) -> (
    // args e.g. &[`_0`],  &[`_0: i32`, `_1: i64`]
    Vec<proc_macro2::TokenStream>,
    // tupled e.g. `_0`, `(_0, _1)`
    proc_macro2::TokenStream,
    // untupled e.g. &[`_0`], &[`_0`, `_1`]
    Vec<proc_macro2::TokenStream>,
    // ty e.g. `Foo`, `(i32, i64)`
    proc_macro2::TokenStream,
) {
    if inputs.len() == 1 {
        let ty = &inputs[0].ty;

        (
            vec![quote!(_0: #ty)],
            quote!(_0),
            vec![quote!(_0)],
            quote!(#ty),
        )
    } else {
        let mut args = vec![];
        let mut pats = vec![];
        let mut tys = vec![];

        for (i, input) in inputs.iter().enumerate() {
            let i = Ident::new(&format!("_{}", i), Span::call_site());
            let ty = &input.ty;

            args.push(quote!(#i: #ty));

            pats.push(quote!(#i));

            tys.push(quote!(#ty));
        }

        let tupled = {
            let pats = pats.clone();
            quote!((#(#pats,)*))
        };
        let ty = quote!((#(#tys,)*));
        (args, tupled, pats, ty)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Idle(u8),
    Init(u8),
    Interrupt(Ident),
    Task(Ident),
}

impl Kind {
    fn ident(&self) -> Ident {
        let span = Span::call_site();
        match self {
            Kind::Idle(..) => Ident::new("idle", span),
            Kind::Init(..) => Ident::new("init", span),
            Kind::Interrupt(name) => name.clone(),
            Kind::Task(name) => name.clone(),
        }
    }

    fn locals_ident(&self) -> Ident {
        Ident::new(&format!("{}Locals", self.ident()), Span::call_site())
    }

    fn resources_ident(&self) -> Ident {
        Ident::new(&format!("{}Resources", self.ident()), Span::call_site())
    }

    fn cfg_core(&self, app: &App) -> Option<proc_macro2::TokenStream> {
        let core = match self {
            Kind::Idle(core) => *core,
            Kind::Init(core) => *core,
            Kind::Interrupt(name) => app.interrupts[name].args.core,
            Kind::Task(name) => app.tasks[name].args.core,
        };

        app.cfg_core(core)
    }

    fn resources<'a>(&self, app: &'a App) -> &'a Idents {
        match self {
            Kind::Init(core) => {
                &app.mains[usize::from(*core)]
                    .init
                    .as_ref()
                    .expect("UNREACHABLE")
                    .args
                    .resources
            }

            Kind::Idle(core) => {
                &app.mains[usize::from(*core)]
                    .idle
                    .as_ref()
                    .expect("UNREACHABLE")
                    .args
                    .resources
            }

            Kind::Interrupt(name) => &app.interrupts[name].args.resources,

            Kind::Task(name) => &app.tasks[name].args.resources,
        }
    }

    fn returns_late_resources(&self, app: &App) -> bool {
        match self {
            Kind::Init(core) => {
                app.mains[usize::from(*core)]
                    .init
                    .as_ref()
                    .expect("UNREACHABLE")
                    .returns_late_resources
            }

            _ => false,
        }
    }

    fn spawn<'a>(&self, app: &'a App) -> &'a Idents {
        match self {
            Kind::Init(core) => {
                &app.mains[usize::from(*core)]
                    .init
                    .as_ref()
                    .expect("UNREACHABLE")
                    .args
                    .spawn
            }

            Kind::Idle(core) => {
                &app.mains[usize::from(*core)]
                    .idle
                    .as_ref()
                    .expect("UNREACHABLE")
                    .args
                    .spawn
            }

            Kind::Interrupt(name) => &app.interrupts[name].args.spawn,

            Kind::Task(name) => &app.tasks[name].args.spawn,
        }
    }

    fn is_idle(&self) -> bool {
        match *self {
            Kind::Idle(_) => true,
            _ => false,
        }
    }

    fn is_init(&self) -> bool {
        match *self {
            Kind::Init(_) => true,
            _ => false,
        }
    }

    fn runs_once(&self) -> bool {
        self.is_init() || self.is_idle()
    }

    fn doc(&self) -> &str {
        match self {
            Kind::Idle(..) => "Idle loop",
            Kind::Init(..) => "Initialization function",
            Kind::Interrupt(..) => "Hardware task",
            Kind::Task(..) => "Software task",
        }
    }
}
