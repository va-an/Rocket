use std::path::Path;

use handlebars::Handlebars;
use rocket::serde::Serialize;

use crate::engine::Engine;

impl Engine for Handlebars<'static> {
    const EXT: &'static str = "hbs";

    fn init<'a>(templates: impl Iterator<Item = (&'a str, &'a Path)>) -> Option<Self> {
        let mut hb = Handlebars::new();
        let mut ok = true;
        for (template, path) in templates {
            if let Err(e) = hb.register_template_file(template, path) {
                error!(template, path = %path.display(),
                    "failed to register Handlebars template: {e}");

                ok = false;
            }
        }

        ok.then_some(hb)
    }

    fn render<C: Serialize>(&self, template: &str, context: C) -> Option<String> {
        if self.get_template(template).is_none() {
            error!(template, "requested Handlebars template does not exist.");
            return None;
        }

        Handlebars::render(self, template, &context)
            .map_err(|e| error!("Handlebars render error: {}", e))
            .ok()
    }
}
