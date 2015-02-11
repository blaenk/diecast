#![feature(plugin)]
#![feature(path)]
#![feature(io)]
#![feature(core)]

#[plugin]
extern crate diecast;
#[plugin] #[no_link]
extern crate regex_macros;
extern crate glob;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate hoedown;
extern crate handlebars;
extern crate "rustc-serialize" as rustc_serialize;

use diecast::{
    Diecast,
    // Site,
    Configuration,
    Rule,
    Compiler,
    Chain,
    Item,
    Dependencies,
};

use diecast::router;
use diecast::compiler::{self, TomlMetadata};
use hoedown::buffer::Buffer;

use handlebars::Handlebars;
use std::old_io::fs::File;
use std::collections::BTreeMap;
use std::sync::Arc;
use rustc_serialize::json::{Json, ToJson};

fn article_handler(item: &Item, _deps: Option<Dependencies>) -> Json {
    let mut bt: BTreeMap<String, Json> = BTreeMap::new();

    if let Some(&TomlMetadata(ref metadata)) = item.data.get::<TomlMetadata>() {
        if let Some(body) = item.data.get::<Buffer>() {
            bt.insert("body".to_string(), body.as_str().unwrap().to_json());
        }

        if let Some(title) = metadata.lookup("title") {
            bt.insert("title".to_string(), title.as_str().unwrap().to_json());
        }
    }

    Json::Object(bt)
}

fn main() {
    env_logger::init().unwrap();

    let layout =
        File::open(&Path::new("tests/fixtures/input/layouts/article.handlebars"))
            .read_to_string().unwrap();

    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("article", layout).unwrap();

    let template_registry = Arc::new(handlebars);

    let content_compiler =
        Compiler::new(
            Chain::new()
                // .link(compiler::Inject::with(template_registry))
                .link(compiler::inject_with(template_registry))
                .link(compiler::read)
                .link(compiler::parse_metadata)
                .link(compiler::parse_toml)
                .link(compiler::render_markdown)
                // .link(router::SetExtension::new("html"))
                .link(router::set_extension("html"))
                // .link(compiler::RenderTemplate::new("article", article_handler))
                .link(compiler::render_template("article", article_handler))
                .link(compiler::print)
                .link(compiler::write)
                .build());

    let posts =
        Rule::new("posts")
            .compiler(content_compiler.clone());

    let config =
        Configuration::new(Path::new("tests/fixtures/input"), Path::new("output"))
            // .preview(true)
            .ignore(regex!(r"^\.|^#|~$|\.swp$"));

    // let site =
    //     Site::new(config)
    //         .matching(glob::Pattern::new("pages/*.md").unwrap(), posts);

    // approach
    //
    // 1. Rule itself contains matching/create
    //
    //     Rule::matching("posts", glob::Pattern::new("pages/*.md").unwrap())
    //     Rule::creating("post index", "index.html")
    //
    // 2. pass rules to Diecast
    //

    Diecast::new(config)
        .rule(posts)
        .run();
}

