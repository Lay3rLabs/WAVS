use crate::context::CliContext;
use anyhow::Result;

pub struct TempCallScript {
    pub args: forge_script::ScriptArgs,
}
impl TempCallScript {
    pub async fn run(_ctx: &CliContext, args: forge_script::ScriptArgs) -> Result<Self> {
        args.clone()
            .run_script()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to run script: {}", e))?;

        Ok(Self { args })
    }
}

impl std::fmt::Display for TempCallScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.args)?;
        Ok(())
    }
}
