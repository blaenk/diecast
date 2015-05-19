//! Site generation.

use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashSet;

use job::{self, Job};
use configuration::Configuration;
use rule::Rule;
use support;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,
    rules: Vec<Arc<Rule>>,
    manager: job::Manager<job::evaluator::Pool<Job>>,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        let queue = job::evaluator::Pool::new(4);

        let configuration = Arc::new(configuration);
        let manager = job::Manager::new(queue, configuration.clone());

        Site {
            configuration: configuration,
            rules: Vec::new(),
            manager: manager,
        }
    }

    fn configure(&mut self, configuration: Configuration) {
        self.configuration = Arc::new(configuration);
    }

    fn prepare(&mut self) {
        trace!("finding jobs");

        trace!("output directory is: {:?}", self.configuration.output);

        if !support::file_exists(&self.configuration.input) {
            println!("the input directory `{:?}` does not exist!", self.configuration.input);
            ::std::process::exit(1);
        }

        for rule in &self.rules {
           // FIXME: this just seems weird re: strings
           self.manager.add(rule.clone());
        }

        trace!("creating output directory at {:?}", &self.configuration.output);

        // create the output directory
        support::mkdir_p(&self.configuration.output).unwrap();

        // TODO: use resolve_from for partial builds?
        trace!("resolving graph");
    }

    pub fn build(&mut self) {
        // TODO: clean out the output directory here to avoid cruft and conflicts
        // trace!("cleaning out directory");
        self.clean();

        self.prepare();
        self.manager.build();
    }

    pub fn update(&mut self, paths: HashSet<PathBuf>) {
        self.prepare();
        self.manager.update(paths);
    }

    pub fn register(&mut self, rule: Rule) {
        if !rule.dependencies().is_empty() {
            let names =
                self.rules.iter().map(|r| String::from(r.name())).collect();
            let diff: HashSet<_> =
                rule.dependencies().difference(&names).cloned().collect();

            if !diff.is_empty() {
                println!("`{}` depends on unregistered rule(s) `{:?}`", rule.name(), diff);
                ::std::process::exit(1);
            }
        }

        self.rules.push(Arc::new(rule));
    }

    pub fn configuration(&self) -> Arc<Configuration> {
        self.configuration.clone()
    }

    pub fn clean(&self) {
        use std::fs::{
            read_dir,
            remove_dir_all,
            remove_file,
        };

        trace!("cleaning");

        if !support::file_exists(&self.configuration.output) {
            return;
        }

        // TODO: probably don't need ignore hidden?
        // TODO: maybe obey .gitignore?
        // clear directory
        for child in read_dir(&self.configuration.output).unwrap() {
            let path = child.unwrap().path();

            if !self.configuration.ignore_hidden ||
                path.file_name().unwrap()
                    .to_str().unwrap()
                    .chars().next().unwrap() != '.' {
                if ::std::fs::metadata(&path).unwrap().is_dir() {
                    remove_dir_all(&path).unwrap();
                } else {
                    remove_file(&path).unwrap();
                }
            }
        }
    }
}

