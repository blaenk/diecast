use std::sync::Arc;
use std::path::PathBuf;
use std::fmt;

use bind::{self, Bind};
use handle::Handle;
use rule;

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind_data: bind::Data,
    pub kind: Arc<rule::Kind>,
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    pub bind: Option<Bind>,
    paths: Arc<Vec<PathBuf>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind_data.name)
    }
}

impl Job {
    pub fn new(
        bind: bind::Data,
        kind: Arc<rule::Kind>,
        handler: Arc<Box<Handle<Bind> + Sync + Send>>,
        paths: Arc<Vec<PathBuf>>)
    -> Job {
        Job { bind_data: bind, kind: kind, handler: handler, bind: None, paths: paths }
    }

    // TODO
    pub fn into_bind(self) -> Bind {
        self.bind.unwrap()
    }

    // TODO: feels weird to have this here
    fn populate(&self, bind: &mut Bind) {
        use item::Route;
        use support;

        // TODO: bind.spawn(Route::Read(relative))
        // let data = bind.data();

        match *self.kind {
            rule::Kind::Creating => (),
            rule::Kind::Matching(ref pattern) => {
                for path in self.paths.iter() {
                    let relative =
                        support::path_relative_from(path, &bind.configuration.input).unwrap()
                        .to_path_buf();

                    // TODO: JOIN STANDARDS
                    // should insert path.clone()
                    if pattern.matches(&relative) {
                        let item = bind.spawn(Route::Read(relative));
                        bind.items_mut().push(item);
                    }
                }
            },
        }
    }

    pub fn process(&mut self) -> ::Result {
        use ansi_term::Colour::Green;
        use ansi_term::Style;

        fn action(bind: &Bind) -> &'static str {
            if bind.is_stale() {
                ::UPDATING
            } else {
                ::STARTING
            }
        }

        fn item_count(bind: &Bind) -> usize {
            if bind.is_stale() {
                bind.iter().count()
            } else {
                bind.items().len()
            }
        }

        if let Some(ref mut bind) = self.bind {
            println!("{} {}",
                Green.bold().paint(action(&bind)),
                bind);

            let res = self.handler.handle(bind);

            println!("{} {} ({} items)",
                Style::default().bold().paint(::FINISHED),
                bind,
                item_count(&bind));

            res
        } else {
            // TODO I don't think this branch could possibly be an update
            // optimize by removing that dynamic check
            let mut bind =
                Bind::new(self.bind_data.clone());

            // populate with items
            self.populate(&mut bind);

            println!("{} {}",
                Green.bold().paint(action(&bind)),
                bind);

            // TODO: rust-pad patch to take Deref<Target=str> or AsRef<str>?
            let res = self.handler.handle(&mut bind);

            println!("{} {} ({} items)",
                Style::default().bold().paint(::FINISHED),
                bind,
                item_count(&bind));

            self.bind = Some(bind);

            res
        }
    }
}

