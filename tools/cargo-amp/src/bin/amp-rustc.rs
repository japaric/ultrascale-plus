//! Wrapper around `rustc` used to build shared AMP data

use std::{
    env,
    process::{self, Command},
};

use exitfailure::ExitFailure;

fn main() -> Result<(), ExitFailure> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    if args.windows(2).any(|s| s == ["--cfg", "amp_data"]) {
        let mut inc = None;
        let mut next_is_crate_type = false;
        for (i, arg) in args.iter_mut().enumerate() {
            if next_is_crate_type {
                next_is_crate_type = false;
                *arg = "lib".to_owned();
            } else if arg.starts_with("--emit=") {
                *arg = "--emit=dep-info,obj".to_owned();
            } else if arg.starts_with("incremental=") {
                inc = Some(i - 1);
            } else if arg == "--crate-type" {
                next_is_crate_type = true;
            }
        }

        // incremental causes problems so drop it
        if let Some(i) = inc {
            // -C
            args.remove(i);
            // incremental=..
            args.remove(i);
        }

        // don't emit dead code and other warnings
        args.push("-A".to_owned());
        args.push("warnings".to_owned());
    }

    let status = Command::new("rustc").args(args).status()?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1))
    }

    Ok(())
}
