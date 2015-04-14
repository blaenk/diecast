use std::sync::Arc;

use regex::Regex;
use rustc_serialize::json::Json;
use handlebars::Handlebars;
use toml;

use handle::{self, Handle, Result};
use item::Item;

use super::{Chain, Injector};

impl Handle<Item> for Chain<Item> {
    fn handle(&self, item: &mut Item) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl<T> Handle<Item> for Injector<T> where T: Sync + Send + Clone + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.data.insert(self.payload.clone());
        Ok(())
    }
}

pub fn copy(item: &mut Item) -> handle::Result {
    use std::fs;

    if let Some(from) = item.reading() {
        if let Some(to) = item.writing() {
            // TODO: once path normalization is in, make sure
            // writing to output folder

            if let Some(parent) = to.parent() {
                // TODO: this errors out if the path already exists? dumb
                let _ = fs::create_dir_all(parent);
            }

            try!(fs::copy(from, to));
        }
    }

    Ok(())
}

/// Handle<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> handle::Result {
    use std::fs::File;
    use std::io::Read;

    let mut buf = String::new();

    if let Some(from) = item.reading() {
        // TODO: use try!
        File::open(from)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
    }

    item.body = buf;

    Ok(())
}

/// Handle<Item> that writes the `Item`'s body.
pub fn write(item: &mut Item) -> handle::Result {
    use std::fs::{self, File};
    use std::io::Write;

    if let Some(to) = item.writing() {
        // TODO: once path normalization is in, make sure
        // writing to output folder

        if let Some(parent) = to.parent() {
            // TODO: this errors out if the path already exists? dumb
            let _ = fs::create_dir_all(parent);
        }

        trace!("writing file {:?}", to);

        File::create(&to)
            .unwrap()
            .write_all(item.body.as_bytes())
            .unwrap();
    }

    Ok(())
}


/// Handle<Item> that prints the `Item`'s body.
pub fn print(item: &mut Item) -> handle::Result {
    println!("{}", item.body);

    Ok(())
}

#[derive(Clone)]
pub struct Metadata {
    pub data: toml::Value,
}

pub fn parse_metadata(item: &mut Item) -> handle::Result {
    // TODO:
    // should probably allow arbitrary amount of
    // newlines after metadata block?
    let re =
        Regex::new(
            concat!(
                "(?ms)",
                r"\A---\s*\n",
                r"(?P<metadata>.*?\n?)",
                r"^---\s*$",
                r"\n*",
                r"(?P<body>.*)"))
            .unwrap();

    let body = if let Some(captures) = re.captures(&item.body) {
        if let Some(metadata) = captures.name("metadata") {
            if let Ok(parsed) = metadata.parse() {
                item.data.insert(Metadata { data: parsed });
            }
        }

        captures.name("body").map(|b| b.to_string())
    } else { None };

    if let Some(body) = body {
        item.body = body;
    }

    Ok(())
}

pub fn render_markdown(item: &mut Item) -> handle::Result {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    let document = Markdown::new(item.body.as_bytes());
    let renderer = html::Html::new(html::Flags::empty(), 0);
    let buffer = document.render_to_buffer(renderer);
    item.data.insert(buffer);

    Ok(())
}

pub struct RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    name: &'static str,
    handler: H,
}

impl<H> Handle<Item> for RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.body = {
            let data =
                item.bind().dependencies["templates"]
                .data().data.read().unwrap();
            let registry = data.get::<Arc<Handlebars>>().unwrap();

            trace!("rendering template for {:?}", item);
            let json = (self.handler)(item);

            registry.render(self.name, &json).unwrap()
        };

        Ok(())
    }
}

#[inline]
pub fn render_template<H>(name: &'static str, handler: H) -> RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    RenderTemplate {
        name: name,
        handler: handler,
    }
}

pub fn is_draft(item: &Item) -> bool {
    item.data.get::<Metadata>()
        .map(|meta| {
            meta.data.lookup("draft")
                .and_then(::toml::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.bind().configuration.is_preview)
}
