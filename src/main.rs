#[allow(unused_imports)]
extern crate console;
#[macro_use] extern crate log;
extern crate env_logger;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate clap;
extern crate reqwest;

mod djs;

use console::{style};
use std::env::home_dir;
use djs::download::download;
use djs::mediator::Mediator;
use djs::console::ConsoleMediator;
use std::io::{stderr, Write};
use std::process::{exit};
use djs::config::{Config, validate_config};
use djs::cli::{configure_from_cli, build_cli};
use djs::rc::{configure_from_file};
use djs::jenkins::Jenkins;
use djs::git::guess_branch;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;

macro_rules! dump_configm {
    ($mediator:ident, $config: ident, $title:expr, $opt:ident) => {
        let value = match $config.$opt().len() {
            0 => "<empty>".to_string(),
            _ => $config.$opt()
        };
        $mediator.print(format!("{} ({}): {}",
                               $title,
                               stringify!($opt),
                               style(value).green(),
                               ));
    }
}

macro_rules! dump_config {
    ($mediator:ident, $config: ident, $title:expr, $opt:ident) => {
        let value = match $config.$opt.get().len() {
            0 => "<empty>".to_string(),
            _ => $config.$opt.get()
        };
        $mediator.print(format!("{} ({}): {} [source: {}]",
                               $title,
                               stringify!($opt),
                               style(value).green(),
                               style($config.$opt.source()).magenta(),
                               ));
    }
}

fn main() {
    #![allow(unused_must_use)]
    env_logger::init();
    let cli = build_cli();
	let opts = cli.get_matches();
    let config = Rc::new(RefCell::new(Config {..Default::default()}));

    debug!("initial config={:?}", config.borrow());

    if let Some(mut home_pb) = home_dir() {
        home_pb.push(".djsrc");
        configure_from_file(home_pb.as_path(), Rc::clone(&config));
    }
    configure_from_file(Path::new("./.djsrc"), Rc::clone(&config));

    // start from the default config
    // then 'guess' the git branch
    //   if it's specfiied in the file or local .rc file then we ignore the branch
    //
    if let Some(git_branch) = guess_branch() {
        debug!("Guessed git branch is {:?}", git_branch);
        config.borrow_mut().branch.set(git_branch, String::from("git"));
    }
    // read from file
    // override from command line


    debug!("About to configure from CLI");
    configure_from_cli(Rc::clone(&config), &opts).expect("Failed to parse the CLI");


    if let Some(err) = validate_config(Rc::clone(&config)).err() {
        writeln!(stderr(), "{:?}", err).unwrap();
        exit(1)
    }

    // i don't like this.  the mediator only needs to read from the config
    // while the jenkins struct needs to modify it
    let mut mediator = ConsoleMediator::new(Rc::clone(&config));

    let mut j = Jenkins::new(Rc::clone(&config));
    debug!("Jenkins = {:?}", j);

    let resolved_url = j.resolve_download_url();
    if config.borrow().verbose.get() {
        let config_snapshot = config.borrow();
       dump_config!(mediator, config_snapshot,"Jenkins Base URL", url);
       dump_config!(mediator, config_snapshot,"Jenkins Base Path", base);
       dump_config!(mediator, config_snapshot,"Project", project);
       dump_config!(mediator, config_snapshot,"Branch", branch);
       dump_config!(mediator, config_snapshot,"Build", build);
       dump_config!(mediator, config_snapshot,"Solution", solution);
       dump_config!(mediator, config_snapshot,"Solution Filter", solution_filter);
       dump_config!(mediator, config_snapshot,"Destination", destination);
       dump_configm!(mediator, config_snapshot,"Destination Path", destination_path);
    }

    let download_result = resolved_url.and_then(|url| {
            mediator.print(format!("Resolved URL: {}", style(url.as_str()).green()));
            if !config.borrow().dry_run.get() {
                let destination_path = config.borrow().destination_path();

                download(url.as_str(), destination_path.as_str(), &mut mediator)
                    .map_err(|e| String::from(e.description()))
            } else {
                mediator.print(format!("Dry Run, not downloading the file."));
                Ok(())
            }
        });

    match download_result {
        Err(err) => {
            writeln!(stderr(), "{} {}",style("ERROR").bold().red(), style(err).white());
            exit(1)
        },
        Ok(_) => {
            mediator.print(String::from("Done"));
            exit(0)
        }
    }
}

