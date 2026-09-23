use anyhow::{bail, Result};
use serde::Serialize;

use crate::cli::ContextArgs;
use crate::index::{
    EventRecord, LocalSessionSource, MessageRecord, NativeThreadHandoff, Store, ThreadContextRead,
    ThreadRecord,
};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct ThreadContextResponse {
    command: &'static str,
    ok: bool,
    thread: ThreadRecord,
    source: LocalSessionSource,
    handoff: NativeThreadHandoff,
    budget: ContextBudget,
    messages: Vec<MessageRecord>,
    events: Vec<EventRecord>,
    text: String,
}

#[derive(Debug, Serialize)]
struct ContextBudget {
    unit: &'static str,
    applies_to: &'static str,
    limit: usize,
    used: usize,
}

pub fn thread(store: &Store, args: &ContextArgs) -> Result<Rendered> {
    let include_events = !args.no_events;
    let context = store.read_thread_context(
        &args.session_id,
        Some(args.messages),
        if include_events {
            Some(args.events)
        } else {
            Some(0)
        },
    )?;
    let text = render_context(&context, args.budget, include_events)?;
    let source = LocalSessionSource::new(Some(context.thread.path.clone()));
    let handoff = NativeThreadHandoff::new(&context.thread.session_id);
    let response = ThreadContextResponse {
        command: "threads.context",
        ok: true,
        thread: context.thread,
        source,
        handoff,
        budget: ContextBudget {
            unit: "utf8_bytes",
            applies_to: "text",
            limit: args.budget,
            used: text.len(),
        },
        messages: context.messages,
        events: if include_events {
            context.events
        } else {
            Vec::new()
        },
        text: text.clone(),
    };

    Rendered::new(text, &response)
}

fn render_context(
    context: &ThreadContextRead,
    budget: usize,
    include_events: bool,
) -> Result<String> {
    let resume_pointers = render_resume_pointers(&context.thread.session_id);
    if resume_pointers.len() > budget {
        bail!(
            "--budget 至少需要 {} 字节以容纳 Resume Pointers",
            resume_pointers.len()
        );
    }
    let content_budget = budget - resume_pointers.len();
    let mut builder = BudgetedText::new(content_budget);
    builder.push("# Codex Thread Context\n");
    builder.push(&format!(
        "- session_id: {}\n- title: {}\n",
        context.thread.session_id, context.thread.title
    ));
    if let Some(cwd) = context.thread.cwd.as_deref() {
        builder.push(&format!("- cwd: {}\n", cwd));
    }
    builder.push(&format!(
        "- messages: {}\n- events: {}\n\n",
        context.thread.message_count, context.thread.event_count
    ));

    builder.push("## Recent Messages\n");
    for message in &context.messages {
        builder.push(&format!(
            "- {} {}: {}\n",
            message.timestamp.as_deref().unwrap_or(""),
            message.role,
            trim_item(&message.text, 260)
        ));
    }

    if include_events {
        builder.push("\n## Execution Evidence\n");
        for event in &context.events {
            builder.push(&format!(
                "- {} {}: {}\n",
                event.timestamp.as_deref().unwrap_or(""),
                event.event_type,
                trim_item(&event.summary, 220)
            ));
        }
    }

    let mut text = builder.finish();
    if !text.ends_with('\n') && text.len() < content_budget {
        text.push('\n');
    }
    text.push_str(&resume_pointers);
    Ok(text)
}

fn render_resume_pointers(session_id: &str) -> String {
    format!(
        "\n## Resume Pointers\n- Confirm candidate thread ID with Codex native threads: {}\n- Local fallback: codex-threads threads read {} --limit 20\n- Read event trail: codex-threads events read {} --limit 20\n",
        session_id,
        session_id, session_id
    )
}

struct BudgetedText {
    limit: usize,
    text: String,
}

impl BudgetedText {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            text: String::new(),
        }
    }

    fn push(&mut self, value: &str) {
        if self.text.len() >= self.limit {
            return;
        }
        let remaining = self.limit - self.text.len();
        if value.len() <= remaining {
            self.text.push_str(value);
        } else {
            self.text.push_str(&take_chars(value, remaining));
        }
    }

    fn finish(self) -> String {
        self.text
    }
}

fn trim_item(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    format!(
        "{}...",
        normalized.chars().take(max_chars).collect::<String>()
    )
}

fn take_chars(text: &str, max_bytes: usize) -> String {
    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    text[..end].to_string()
}
