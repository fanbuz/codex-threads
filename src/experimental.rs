use std::collections::BTreeSet;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExperimentalFeature {
    RestoreAppThread,
}

impl ExperimentalFeature {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "restore-app-thread" => Ok(Self::RestoreAppThread),
            other => bail!(
                "未知实验能力: {}。当前可用值: {}",
                other,
                Self::available_values().join(", ")
            ),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RestoreAppThread => "restore-app-thread",
        }
    }

    fn available_values() -> Vec<&'static str> {
        vec![Self::RestoreAppThread.as_str()]
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExperimentalFeatures {
    enabled: BTreeSet<ExperimentalFeature>,
}

impl ExperimentalFeatures {
    pub fn parse_csv(raw: Option<&str>) -> Result<Self> {
        let Some(raw) = raw else {
            return Ok(Self::default());
        };

        let mut enabled = BTreeSet::new();
        for feature in raw.split(',') {
            let normalized = feature.trim();
            if normalized.is_empty() {
                bail!(
                    "--enable-experimentals 里不能包含空的 feature 名，请使用 --enable-experimentals restore-app-thread"
                );
            }
            enabled.insert(ExperimentalFeature::parse(normalized)?);
        }

        Ok(Self { enabled })
    }

    pub fn ensure_enabled(&self, feature: ExperimentalFeature) -> Result<()> {
        if self.enabled.contains(&feature) {
            return Ok(());
        }

        bail!(
            "这个能力当前仍处于实验阶段。请显式添加 --enable-experimentals {} 后再执行。",
            feature.as_str()
        )
    }
}
