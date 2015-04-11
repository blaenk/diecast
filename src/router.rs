use item::{self, Item};
use std::path::{PathBuf, Path};
use compiler;

use regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

/// file.txt -> file.txt
/// gen.route(Identity)
pub fn identity(item: &mut Item) {
    item.route(|path: &Path| -> PathBuf {
        trace!("routing {} with the identity router", path.display());
        path.to_path_buf()
    });
}

pub fn set_extension(extension: &'static str) -> Box<item::Handler + Sync + Send> {
    Box::new(move |item: &mut Item| -> compiler::Result {
        item.route(|path: &Path| -> PathBuf {
            path.with_extension(extension)
        });

        Ok(())
    })
}

/// file.txt -> file.html
/// gen.route(SetExtension::new("html"))
#[derive(Copy, Clone)]
pub struct SetExtension {
    extension: &'static str,
}

impl SetExtension {
    pub fn new(extension: &'static str) -> SetExtension {
        SetExtension {
            extension: extension,
        }
    }
}

impl item::Handler for SetExtension {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        item.route(|path: &Path| -> PathBuf {
            path.with_extension(self.extension)
        });

        Ok(())
    }
}

/// regex expansion
///
/// gen.route(
///     RegexRoute::new(
///         regex!("/posts/post-(?P<name>.+)\.markdown"),
///         "/target/$name.html"));
#[derive(Clone)]
pub struct Regex {
    regex: regex::Regex,

    // perhaps use regex::Replacer instead?
    // http://doc.rust-lang.org/regex/regex/trait.Replacer.html
    template: &'static str,
}

impl Regex {
    pub fn new(regex: regex::Regex, template: &'static str) -> Regex {
        Regex {
            regex: regex,
            template: template,
        }
    }
}

impl item::Handler for Regex {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        item.route(|path: &Path| -> PathBuf {
            let caps = self.regex.captures(path.to_str().unwrap()).unwrap();
            PathBuf::from(&caps.expand(self.template))
        });

        Ok(())
    }
}

