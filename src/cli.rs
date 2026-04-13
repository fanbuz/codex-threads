use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "codex-threads",
    version,
    about = "索引、搜索和读取 Codex 历史线程",
    after_help = concat!("CLI 版本: ", env!("CARGO_PKG_VERSION"))
)]
pub struct Cli {
    #[arg(long, global = true, help = "输出结构化 JSON")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "会话目录，默认 ~/.codex/sessions"
    )]
    pub sessions_dir: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "索引目录，默认 ~/.codex/threads-index"
    )]
    pub index_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "增量扫描会话文件并更新索引")]
    Sync(SyncArgs),
    #[command(about = "查看索引状态和统计信息")]
    Status,
    #[command(about = "搜索和读取线程")]
    Threads {
        #[command(subcommand)]
        command: ThreadsCommand,
    },
    #[command(about = "搜索和读取消息")]
    Messages {
        #[command(subcommand)]
        command: MessagesCommand,
    },
    #[command(about = "搜索和读取事件记录")]
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
}

#[derive(Debug, Args, Clone, Default)]
pub struct SyncArgs {
    #[arg(long, value_name = "SINCE", help = "只同步不早于该时间的会话文件")]
    pub since: Option<String>,

    #[arg(long, value_name = "UNTIL", help = "只同步不晚于该时间的会话文件")]
    pub until: Option<String>,

    #[arg(long, value_name = "PATH", help = "只同步路径命中该片段的会话文件")]
    pub path: Option<String>,

    #[arg(long, value_name = "RECENT", help = "只同步最近活跃的 N 个会话文件")]
    pub recent: Option<usize>,
}

#[derive(Debug, Subcommand)]
pub enum ThreadsCommand {
    #[command(about = "按标题、路径和聚合内容搜索线程")]
    Search(ThreadSearchArgs),
    #[command(about = "读取指定线程")]
    Read(ReadArgs),
}

#[derive(Debug, Subcommand)]
pub enum MessagesCommand {
    #[command(about = "在所有历史消息中搜索关键词")]
    Search(MessageSearchArgs),
    #[command(about = "读取指定线程里的消息")]
    Read(ReadArgs),
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    #[command(about = "在所有历史事件中搜索关键词")]
    Search(EventSearchArgs),
    #[command(about = "读取指定线程里的事件记录")]
    Read(ReadArgs),
}

#[derive(Debug, Args, Clone)]
pub struct CommonSearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 20, help = "最多返回多少条结果")]
    pub limit: usize,

    #[arg(long, value_name = "SINCE", help = "只返回不早于该时间的结果")]
    pub since: Option<String>,

    #[arg(long, value_name = "UNTIL", help = "只返回不晚于该时间的结果")]
    pub until: Option<String>,

    #[arg(long, value_name = "SESSION", help = "只返回指定 session_id 的结果")]
    pub session: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ThreadSearchArgs {
    #[command(flatten)]
    pub common: CommonSearchArgs,

    #[arg(long, value_name = "CWD", help = "按工作目录模糊过滤线程")]
    pub cwd: Option<String>,

    #[arg(long, value_name = "PATH", help = "按线程文件路径模糊过滤线程")]
    pub path: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct MessageSearchArgs {
    #[command(flatten)]
    pub common: CommonSearchArgs,

    #[arg(long, value_name = "ROLE", help = "只返回指定角色的消息")]
    pub role: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct EventSearchArgs {
    #[command(flatten)]
    pub common: CommonSearchArgs,

    #[arg(long, value_name = "EVENT_TYPE", help = "只返回指定类型的事件")]
    pub event_type: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ReadArgs {
    pub session_id: String,

    #[arg(long, help = "只读取最近 N 条记录")]
    pub limit: Option<usize>,
}
