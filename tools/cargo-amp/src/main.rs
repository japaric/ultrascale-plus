use std::{
    collections::BTreeMap,
    env, fs,
    process::{self, Command},
    time::SystemTime,
};

use cargo_project::{Artifact, Profile, Project};
use clap::{App, Arg};
use exitfailure::ExitFailure;
use failure::{bail, ensure};
use xmas_elf::{sections::SectionData, symbol_table::Entry, ElfFile};

fn main() -> Result<(), ExitFailure> {
    process::exit(run()?)
}

fn run() -> Result<i32, failure::Error> {
    let matches = App::new("cargo-call-stack")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jorge Aparicio <jorge@japaric.io>")
        .about("Build AMP (Asymmetric MultiProcessing) programs")
        // as this is used as a Cargo subcommand the first argument will be the name of the binary
        // we ignore this argument
        .arg(Arg::with_name("binary-name").hidden(true))
        .arg(
            Arg::with_name("cores")
                .long("cores")
                .short("c")
                .takes_value(true)
                .value_name("N")
                .help("Number of cores to build this program for (default: 2)"),
        )
        .arg(
            Arg::with_name("target")
                .long("target")
                .takes_value(true)
                .value_name("TRIPLE")
                .help("Target triple for which the code is compiled"),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Use verbose output"),
        )
        .arg(
            Arg::with_name("example")
                .long("example")
                .takes_value(true)
                .value_name("NAME")
                .help("Build only the specified example"),
        )
        .arg(
            Arg::with_name("bin")
                .long("bin")
                .takes_value(true)
                .value_name("BIN")
                .help("Build only the specified binary"),
        )
        .arg(
            Arg::with_name("features")
                .long("features")
                .takes_value(true)
                .value_name("FEATURES")
                .help("Space-separated list of features to activate"),
        )
        .arg(
            Arg::with_name("all-features")
                .long("all-features")
                .takes_value(false)
                .help("Activate all available features"),
        )
        .arg(
            Arg::with_name("release")
                .long("release")
                .help("Build artifacts in release mode, with optimizations"),
        )
        .get_matches();

    let cores = matches
        .value_of("cores")
        .map(str::parse)
        .unwrap_or(Ok(2_usize))?;
    let target_flag = matches.value_of("target");
    let profile = if matches.is_present("release") {
        Profile::Release
    } else {
        Profile::Dev
    };
    let verbose = matches.is_present("verbose");

    let artifact = match (matches.value_of("bin"), matches.value_of("example")) {
        (Some(bin), None) => Artifact::Bin(bin),
        (None, Some(ex)) => Artifact::Example(ex),
        _ => bail!("please specify --example <NAME> or --bin <NAME>"),
    };

    let meta = rustc_version::version_meta()?;
    let host = meta.host;
    let project = Project::query(env::current_dir()?)?;

    let cargo = || {
        let mut c = Command::new("cargo");
        c.arg("rustc");
        match artifact {
            Artifact::Bin(bin) => {
                c.args(&["--bin", bin]);
            }
            Artifact::Example(ex) => {
                c.args(&["--example", ex]);
            }
            _ => {}
        }
        if profile.is_release() {
            c.arg("--release");
        }
        c.arg("--");
        c
    };

    let mut c = cargo();
    c.env("RUSTC", "amp-rustc").args(&["--cfg", "amp_data"]);
    if verbose {
        eprintln!("{:?}", c);
    }
    let status = c.status()?;
    if !status.success() {
        return Ok(status.code().unwrap_or(1));
    }

    let path = project.path(artifact, profile, target_flag, &host)?;
    let parent = path.parent().expect("unreachable");
    let (haystack, name) = match artifact {
        Artifact::Bin(bin) => (parent.join("deps"), bin),
        Artifact::Example(ex) => (parent.to_owned(), ex),
        _ => unreachable!(),
    };

    let prefix = format!("{}-", name.replace('-', "_"));
    let mut so = None;
    let mut mrm = SystemTime::UNIX_EPOCH;
    for e in fs::read_dir(haystack)? {
        let e = e?;
        let p = e.path();

        if p.extension().map(|ext| ext == "o").unwrap_or(false)
            && p.file_stem()
                .expect("unreachable")
                .to_str()
                .expect("unreachable")
                .starts_with(&prefix)
        {
            let modified = e.metadata()?.modified()?;
            if so.is_none() {
                so = Some(p);
                mrm = modified;
            } else {
                if modified > mrm {
                    so = Some(p);
                    mrm = modified;
                }
            }
        }
    }

    let shared_obj = so.expect("unreachable");
    let mut shared_data: Option<Vec<u8>> = None;
    // address -> (size, name)
    let mut shared_symbols: Option<BTreeMap<_, _>> = None;
    for i in 0..cores {
        let mut c = cargo();
        c.args(&[
            "--cfg",
            "amp_shared",
            "--cfg",
            &format!("core=\"{}\"", i),
            "-C",
            &format!("link-arg={}", shared_obj.display()),
        ]);
        if verbose {
            eprintln!("{:?}", c);
        }
        let status = c.status()?;

        if !status.success() {
            return Ok(status.code().unwrap_or(1));
        }

        let filename = format!(
            "{}-{}",
            path.file_name()
                .expect("unreachable")
                .to_str()
                .expect("unreachable"),
            i
        );
        let dst = parent.join(&filename);

        fs::rename(&path, &dst)?;

        let contents = fs::read(&dst)?;
        let elf = ElfFile::new(&contents).map_err(failure::err_msg)?;

        let mut sh_shndx = None;
        for i in 1..elf.header.pt2.sh_count() {
            if let Ok(sh) = elf.section_header(i) {
                if sh.get_name(&elf) == Ok(".shared") {
                    sh_shndx = Some((sh, i));
                    break;
                }
            }
        }

        let (shared, shndx) = if let Some(sh_shndx) = sh_shndx {
            sh_shndx
        } else {
            bail!("({}) `.shared` section is missing", filename);
        };

        if let Some(shared_data) = &shared_data {
            // XXX data maybe be uninitialized so we can't really compare the contents
            // ensure!(
            //     shared.raw_data(&elf) == shared_data.as_slice(),
            //     "({}) contents of the `.shared` section don't match other files'",
            //     filename
            // )
        } else {
            shared_data = Some(shared.raw_data(&elf).to_owned());
        }

        if let Some(symtab) = elf.find_section_by_name(".symtab") {
            match symtab.get_data(&elf).map_err(failure::err_msg)? {
                SectionData::SymbolTable32(entries) => {
                    if let Some(shared_symbols) = &shared_symbols {
                        let symbols: BTreeMap<_, _> = entries
                            .iter()
                            .filter_map(|entry| {
                                if entry.shndx() == shndx {
                                    Some((
                                        entry.value(),
                                        (entry.size(), entry.get_name(&elf).ok().map(String::from)),
                                    ))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        ensure!(
                            &symbols == shared_symbols,
                            "({}) the memory layout of the `.shared` section doesn't \
                             match other files'",
                            filename,
                        );
                    } else {
                        shared_symbols = Some(
                            entries
                                .iter()
                                .filter_map(|entry| {
                                    if entry.shndx() == shndx {
                                        Some((
                                            entry.value(),
                                            (
                                                entry.size(),
                                                entry.get_name(&elf).ok().map(String::from),
                                            ),
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                        )
                    }
                }
                SectionData::SymbolTable64(_) => {
                    bail!("64-bit ELF files are currently unsupported")
                }
                _ => bail!("malformed .symtab section"),
            }
        }
    }

    Ok(0)
}
