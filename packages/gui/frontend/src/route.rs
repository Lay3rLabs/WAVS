use crate::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Logs,
    Services,
    Triggers,
    Submissions,
    Settings,
    NotFound,
}

impl Route {
    pub fn from_url(url: &str) -> Self {
        let url = web_sys::Url::new(url).unwrap_throw();
        let paths = url.pathname();
        let paths = paths
            .split('/')
            // skip all the roots (1 for the domain, 1 for each part of root path)
            .skip(CONFIG.root_path.chars().filter(|c| *c == '/').count() + 1)
            .collect::<Vec<_>>();
        let paths = paths.as_slice();

        // if we need, we can get query params like:
        //let uid = url.search_params().get("uid");

        match paths {
            [""] | [] | ["logs"] => Self::Logs,
            ["settings"] => Self::Settings,
            ["services"] => Self::Services,
            ["triggers"] => Self::Triggers,
            ["submissions"] => Self::Submissions,
            _ => Self::NotFound,
        }
    }

    pub fn link(&self) -> String {
        let s = format!("{}/{}", CONFIG.root_path, self);
        let s = s.trim_end_matches(r#"//"#).to_string();

        s
    }

    pub fn go_to_url(&self) {
        dominator::routing::go_to_url(&self.link());
    }

    #[allow(dead_code)]
    pub fn hard_redirect(&self) {
        let location = web_sys::window().unwrap_throw().location();
        let s: String = self.link();
        location.set_href(&s).unwrap_throw();
    }

    pub fn signal() -> impl Signal<Item = Route> {
        dominator::routing::url()
            .signal_cloned()
            .map(|url| Route::from_url(&url))
    }

    pub fn get() -> Route {
        Route::from_url(&dominator::routing::url().lock_ref())
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: String = match self {
            Route::Logs => "logs".to_string(),
            Route::Settings => "settings".to_string(),
            Route::Services => "services".to_string(),
            Route::Triggers => "triggers".to_string(),
            Route::Submissions => "submissions".to_string(),
            Route::NotFound => "404".to_string(),
        };
        write!(f, "{}", s)
    }
}
