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
    Sync,
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
    #[command(about = "读取事件记录")]
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ThreadsCommand {
    #[command(about = "按标题、路径和聚合内容搜索线程")]
    Search(SearchArgs),
    #[command(about = "读取指定线程")]
    Read(ReadArgs),
}

#[derive(Debug, Subcommand)]
pub enum MessagesCommand {
    #[command(about = "在所有历史消息中搜索关键词")]
    Search(SearchArgs),
    #[command(about = "读取指定线程里的消息")]
    Read(ReadArgs),
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    #[command(about = "读取指定线程里的事件记录")]
    Read(ReadArgs),
}

#[derive(Debug, Args, Clone)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 20, help = "最多返回多少条结果")]
    pub limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct ReadArgs {
    pub session_id: String,

    #[arg(long, help = "只读取最近 N 条记录")]
    pub limit: Option<usize>,
}
