use crate::prelude::*;

pub static CURSOR_POINTER: LazyLock<String> = LazyLock::new(|| {
    class! {
        .style("cursor", "pointer")
    }
});

pub static WORD_WRAP_PRE: LazyLock<String> = LazyLock::new(|| {
    class! {
        .style("white-space", "pre-wrap")
    }
});

pub static SPACE_AFTER: LazyLock<String> = LazyLock::new(|| {
    class! {
        .pseudo!(":after", {
            .style("content", r#"" ""#)
        })
    }
});

pub static TEXT_ALIGN_CENTER: LazyLock<String> = LazyLock::new(|| {
    class! {
        .style("text-align", "center")
    }
});
