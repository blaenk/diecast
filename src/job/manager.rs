use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::{BTreeMap, BTreeSet, VecDeque, HashMap, HashSet};
use std::mem;

use configuration::Configuration;
use dependency::Graph;
use rule::{self, Rule};
use bind::{self, Bind};
use super::evaluator::Evaluator;
use super::Job;

pub struct Manager<E>
where E: Evaluator {
    configuration: Arc<Configuration>,

    rules: HashMap<String, Arc<Rule>>,

    graph: Graph<String>,

    /// the dependency count of each bind
    dependencies: BTreeMap<String, usize>,

    /// a map of binds to the list of jobs that haven't been processed yet
    waiting: VecDeque<Job>,

    /// finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    /// Thread pool to process jobs
    evaluator: E,

    /// number of jobs being managed
    count: usize,

    paths: Arc<Vec<PathBuf>>,
}

/// sample api:
///   manager.add_rule(rule);
///   manager.build();
///
/// later:
///   manager.update_path(path);

impl<E> Manager<E>
where E: Evaluator {
    pub fn new(evaluator: E, configuration: Arc<Configuration>) -> Manager<E> {
        Manager {
            configuration: configuration,
            rules: HashMap::new(),
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: VecDeque::new(),
            finished: BTreeMap::new(),
            // TODO: this is what needs to change afaik
            evaluator: evaluator,
            count: 0,
            paths: Arc::new(Vec::new()),
        }
    }

    pub fn update_paths(&mut self) {
        use walker::Walker;

        let paths =
            Walker::new(&self.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref ignore) = self.configuration.ignore {
                    if ignore.matches(&Path::new(path.file_name().unwrap())) {
                        return None;
                    }
                }

                if ::std::fs::metadata(&path).unwrap().is_file() {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .collect::<Vec<PathBuf>>();

        self.paths = Arc::new(paths);
    }

    pub fn add(&mut self, rule: Arc<Rule>) {
        let data = bind::Data::new(String::from(rule.name()), self.configuration.clone());
        let bind = data.name.clone();

        // TODO: this still necessary?
        // it's only used to determine if anything will actually be done
        // operate on a bind-level
        self.count += 1;

        // if there's no handler then no need to dispatch a job
        // or anything like that
        self.waiting.push_front(Job::new(data, rule.kind().clone(), rule.handler().clone(), self.paths.clone()));

        self.graph.add_node(bind.clone());

        for dep in rule.dependencies() {
            trace!("setting dependency {} -> {}", dep, bind);
            self.graph.add_edge(dep.clone(), bind.clone());
        }

        self.rules.insert(String::from(rule.name()), rule);
    }

    // TODO: will need Borrow bound
    fn satisfy(&mut self, bind: &str) {
        if let Some(dependents) = self.graph.dependents_of(bind) {
            let names = self.dependencies.keys().cloned().collect::<Vec<String>>();

            for name in names {
                if dependents.contains(&name) {
                    *self.dependencies.entry(name).or_insert(0) -= 1;
                }
            }
        }
    }

    fn ready(&mut self) -> VecDeque<Job> {
        let waiting = mem::replace(&mut self.waiting, VecDeque::new());

        let (ready, waiting): (VecDeque<Job>, VecDeque<Job>) =
            waiting.into_iter()
               .partition(|job| self.dependencies[&job.bind_data.name] == 0);

        self.waiting = waiting;

        trace!("the remaining order is {:?}", self.waiting);
        trace!("the ready binds are {:?}", ready);

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        assert!(self.waiting.len() == order.len(), "`waiting` and `order` are not the same length");

        let mut job_map =
            mem::replace(&mut self.waiting, VecDeque::new())
            .into_iter()
            .map(|job| {
                let name = job.bind_data.name.clone();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind_data.name.clone();

                let count = self.graph.dependency_count(&name);
                trace!("{} has {} dependencies", name, count);

                *self.dependencies.entry(name).or_insert(0) += count;

                return job;
            })
            .collect::<VecDeque<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn build(&mut self) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        match self.graph.resolve_all() {
            Ok(order) => {
                self.sort_jobs(order);

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                // TODO: should have some sort of timeout here
                trace!("looping");
                for _ in (0 .. self.count) {
                    match self.evaluator.dequeue() {
                        Some(job) => {
                            trace!("received job from pool");
                            self.handle_done(job);
                        },
                        None => {
                            println!("a job panicked. stopping everything");
                            ::std::process::exit(1);
                        }
                    }
                }
            },
            Err(cycle) => {
                println!("a dependency cycle was detected: {:?}", cycle);
                ::std::process::exit(1);
            },
        }

        self.reset();
    }

    // TODO paths ref
    pub fn update(&mut self, paths: HashSet<PathBuf>) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        let mut matched = vec![];
        let mut didnt = BTreeSet::new();

        // TODO handle deletes and new files
        // * deletes: full build
        // * new files: add Item

        let mut binds = HashMap::new();

        // find the binds that contain the paths
        for bind in self.finished.values() {
            use item;

            let name = bind.data().name.clone();
            let rule = &self.rules[&name];
            let kind = rule.kind().clone();

            let pattern =
                if let rule::Kind::Matching(ref pattern) = *kind {
                    pattern
                } else {
                    continue
                };

            // Borrow<Path> for &PathBuf
            // impl<'a, T, R> Borrow<T> for &'a R where R: Borrow<T>;

            let mut affected =
                paths.iter()
                .filter(|p| pattern.matches(p))
                .cloned()
                .collect::<HashSet<PathBuf>>();

            let is_match = affected.len() > 0;

            // TODO
            // preferably don't clone, instead just modify it in place
            let mut modified: Bind = (**bind).clone();

            for item in modified.items_mut() {
                if item.route().reading().map(|p| affected.remove(p)).unwrap_or(false) {
                    item::set_stale(item, true);
                }
            }

            // paths that were added
            // if affected.len() > 0 {
            //     for path in affected {
            //         bind.push(path);
            //     }
            // }

            bind::set_stale(&mut modified, true);

            if is_match {
                binds.insert(name.clone(), modified);
                matched.push(name);
            } else {
                didnt.insert(name);
            }
        }

        if matched.is_empty() {
            trace!("no binds matched the path");
            return;
        }

        self.waiting.clear();

        // the name of each bind
        match self.graph.resolve(matched) {
            Ok(order) => {
                // create a job for each bind in the order
                for name in &order {
                    let bind = &self.finished[name];
                    let rule = &self.rules[&bind.data().name];

                    let mut job = Job::new(
                        // TODO this might differ from binds bind?
                        bind.data().clone(),
                        rule.kind().clone(),
                        rule.handler().clone(),
                        self.paths.clone());

                    job.bind = binds.remove(name);

                    self.waiting.push_front(job);
                }

                let order_names = order.clone();
                let job_count = order.len();

                self.sort_jobs(order);

                // binds that aren't in the returned order should be assumed
                // to have already been satisfied
                for name in &order_names {
                    if let Some(deps) = self.graph.dependencies_of(name) {
                        let affected = deps.intersection(&didnt).count();
                        *self.dependencies.get_mut(name).unwrap() -= affected;
                    }
                }

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                // TODO: should have some sort of timeout here
                // FIXME
                // can't do while waiting.is_empty() becuase it could
                // be momentarily empty before the rest get added
                trace!("looping");
                for _ in (0 .. job_count) {
                    match self.evaluator.dequeue() {
                        Some(job) => {
                            trace!("received job from pool");
                            self.handle_done(job);
                        },
                        None => {
                            println!("a job panicked. stopping everything");
                            ::std::process::exit(1);
                        }
                    }
                }
            },
            Err(cycle) => {
                println!("a dependency cycle was detected: {:?}", cycle);
                ::std::process::exit(1);
            },
        }

        self.reset();
    }

    // TODO: audit
    fn reset(&mut self) {
        self.graph = Graph::new();
        self.waiting.clear();
        self.count = 0;
    }

    fn handle_done(&mut self, current: Job) {
        trace!("finished {}", current.bind_data.name);
        trace!("before waiting: {:?}", self.waiting);

        let bind = current.bind_data.name.clone();

        // bind is complete
        trace!("bind {} finished", bind);

        // if they're done, move from staging to finished
        self.finished.insert(bind.clone(), Arc::new({
            let mut bind = current.into_bind();
            bind::set_stale(&mut bind, false);
            bind
        }));

        self.satisfy(&bind);
        self.enqueue_ready();
    }

    // TODO: I think this should be part of satisfy
    // one of the benefits of keeping it separate is that
    // we can satisfy multiple binds at once and then perform
    // a bulk enqueue_ready
    fn enqueue_ready(&mut self) {
        for mut job in self.ready() {
            let name = job.bind_data.name.clone();
            trace!("{} is ready", name);

            // use Borrow?
            if let Some(ds) = self.graph.dependencies_of(&name) {
                for dep in ds {
                    trace!("adding dependency: {:?}", dep);
                    job.bind_data.dependencies.insert(dep.clone(), self.finished[dep].clone());
                }
            }

            trace!("job now ready: {:?}", job);

            self.evaluator.enqueue(job);
        }
    }
}

