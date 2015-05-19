use docopt::Docopt;
use configuration::Configuration;

use command::Command;
use site::Site;
use support;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_verbose: bool,
    flag_ignore_hidden: bool,
}

static USAGE: &'static str = "
Usage:
    diecast clean [options]

Options:
    -h, --help            Print this message
    -v, --verbose         Use verbose output
    -i, --ignore-hidden   Don't clean out hidden files and directories

This removes the output directory.
";

pub struct Clean {
    site: Site,
}

impl Clean {
    pub fn new(mut configuration: Configuration) -> Clean {
        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .help(true);

        let options: Options = docopt.decode().unwrap_or_else(|e| {
            e.exit();
        });

        configuration.ignore_hidden = options.flag_ignore_hidden;

        Clean {
            site: Site::new(configuration),
        }
    }

    pub fn plugin(configuration: Configuration) -> Box<Command> {
        Box::new(Clean::new(configuration))
    }
}

impl Command for Clean {
    fn site(&mut self) -> &mut Site {
        &mut self.site
    }

    fn run(&mut self) {
        let target = &self.site.configuration().output;

        if support::file_exists(target) {
            println!("removing {:?}", target);
        } else {
            println!("nothing to remove");
        }

        self.site.clean();
    }
}
