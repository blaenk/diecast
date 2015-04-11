//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::fs::File;
use std::io::{Read, Write};
use std::fmt::{self, Debug};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::{PathBuf, Path};

use binding::{self, Bind};
use compiler;

// TODO:
// pinning down the type like this has the effect of also
// pinning down the evaluation implementation no? this contains Arcs,
// for example, which would nto be necessary in a single threaded evaluator?
// perhaps the alternative is an associated type on a trait
// or perhaps Arcs are fine anyways?
// TODO
// I think this should be its own type
pub type Dependencies = Arc<BTreeMap<String, Arc<Bind>>>;

/// Represents a compilation unit.
///
/// This represents either a file read, a file write, or
/// a mapping from a file read to a file write.
///
/// It includes a body field which represents the read or to-be-written data.
///
/// It also includes an [`AnyMap`](http://www.rust-ci.org/chris-morgan/anymap/doc/anymap/struct.AnyMap.html) which is used to represent miscellaneous data.

// TODO: use a UUID?

#[derive(Clone)]
pub enum Route {
    Read(PathBuf),
    Write(PathBuf),
    ReadWrite(PathBuf, PathBuf),
}

impl Route {
    pub fn reading(&self) -> Option<&Path> {
        match *self {
            Route::Write(_) => None,
            Route::Read(ref path) | Route::ReadWrite(ref path, _) => Some(path),
        }
    }

    pub fn writing(&self) -> Option<&Path> {
        match *self {
            Route::Read(_) => None,
            Route::Write(ref path) | Route::ReadWrite(_, ref path) => Some(path),
        }
    }

    // routing:
    //
    // reading routes to readwrite
    // writing routes to new write
    // readwrite routes to new write
    pub fn route_to<R>(&self, router: R) -> Route
    where R: Fn(&Path) -> PathBuf {
        match *self {
            Route::Read(ref from) => Route::ReadWrite(from.clone(), router(from)),
            Route::Write(ref to) => Route::Write(router(to)),
            Route::ReadWrite(ref from, _) => Route::ReadWrite(from.clone(), router(from)),
        }
    }
}

impl Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Route::Read(ref path) => try!(write!(f, "Reading {}", path.display())),
            Route::Write(ref path) => try!(write!(f, "Writing {}", path.display())),
            Route::ReadWrite(ref from, ref to) => {
                try!(write!(f, "Reading {}, Writing {}", from.display(), to.display()))
            },
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct Item {
    bind: Arc<binding::Data>,

    pub route: Route,

    /// The Item's body which will fill the target file.
    pub body: String,

    /// Any additional data (post metadata)
    ///
    /// * Title
    /// * Date
    /// * Comments
    /// * TOC
    /// * Tags
    pub data: AnyMap,
}

// TODO: Item::from and Item::to
impl Item {
    pub fn new(
        route: Route,
        bind: Arc<binding::Data>,
    ) -> Item {
        use std::fs::PathExt;

        if let Route::Read(ref from) = route {
            println!("from: {:?}", from);
            assert!(bind.configuration.input.join(from).is_file())
        }

        // ensure that the source is a file
        Item {
            route: route,
            body: String::new(),
            data: AnyMap::new(),
            bind: bind,
        }
    }

    pub fn from(path: PathBuf, bind: Arc<binding::Data>) -> Item {
        Item::new(Route::Read(path), bind)
    }

    pub fn to(path: PathBuf, bind: Arc<binding::Data>) -> Item {
        Item::new(Route::Write(path), bind)
    }

    pub fn route<R>(&mut self, router: R)
    where R: Fn(&Path) -> PathBuf {
        self.route = self.route.route_to(router);
    }

    pub fn bind(&self) -> &binding::Data {
        &self.bind
    }

    pub fn read(&mut self) {
        if let Route::Read(ref path) = self.route {
            let mut buf = String::new();

            File::open(&self.bind.configuration.input.join(path))
                .unwrap()
                .read_to_string(&mut buf)
                .unwrap();

            self.body = buf;
        }
    }

    pub fn write(&mut self) {
        if let Route::Write(ref path) = self.route {
            File::create(path)
                .unwrap()
                .write_all(self.body.as_bytes())
                .unwrap();
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.route.fmt(f)
    }
}

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result;
}

impl<H> Handler for Arc<H> where H: Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        (**self).handle(item)
    }
}

impl Handler for Box<Handler> {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        (**self).handle(item)
    }
}

impl Handler for Box<Handler + Sync + Send> {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        (**self).handle(item)
    }
}

impl<F> Handler for F where F: Fn(&mut Item) -> compiler::Result {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        self(item)
    }
}

// TODO: should this be an impl for [H] or for &[H]?
// FIXME: this can't work because a single H type is chosen
//        which ends up expecting all of the elements to be the same type
impl<'a, H> Handler for &'a [H] where H: Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        for handler in *self {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

