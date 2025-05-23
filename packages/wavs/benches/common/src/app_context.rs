use std::sync::LazyLock;

use utils::context::AppContext;

pub static APP_CONTEXT: LazyLock<AppContext> = LazyLock::new(AppContext::new);